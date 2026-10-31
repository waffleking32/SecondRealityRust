//! `cargo xtask compare` end to end: **0** every frame identical, **1** some frame differs,
//! **2** the tool could not compare. A refusal leaves no report on disk. The captures are
//! written here, under the temporary directory.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{scratch, xtask};

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
    // 700/900 against 400/600 holding the same values: zipped positionally they would read
    // `identical`. Nothing is within ±1, so the comparison is refused.
    let reference = scratch("unpairable-reference");
    let rendered = scratch("unpairable-rendered");
    // A path nothing has made yet.
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
    assert!(output.contains("700"), "{output}");
    assert!(output.contains("±1"), "{output}");
    assert!(output.contains("means nothing"), "{output}");
    // Nothing at all, not even the empty output directory.
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
    // `reference/parts`' shape: captures at the top and one level deeper. Every one is
    // measured, and the count is printed with the verdict.
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
    // A run numbered from zero against the reference's indices (`docs/fidelity/tunneli.md`
    // §7.2): the rendered 400/600 are the reference's 700/900 at `--align 300`.
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

    // With it: identical, and the console says the alignment was applied.
    let aligned = run("300");
    assert_eq!(aligned.code, 0, "{}", aligned.output());
    assert!(aligned.output().contains("read +300"), "{}", aligned.output());
    assert!(aligned.output().contains("frame 700: 0.0000%"), "{}", aligned.output());

    // A negative alignment works too.
    let negative_reference = scratch("align-negative-reference");
    let negative_rendered = scratch("align-negative-rendered");
    let negative_out = scratch("align-negative-out").join("reports");
    write_capture(&negative_reference, &[(400, 5), (600, 5)]);
    write_capture(&negative_rendered, &[(700, 5), (900, 5)]);
    let negative = xtask(&[
        "compare",
        "--reference",
        negative_reference.to_str().unwrap(),
        "--rendered",
        negative_rendered.to_str().unwrap(),
        "--out",
        negative_out.to_str().unwrap(),
        "--align",
        "-300",
    ]);
    assert_eq!(negative.code, 0, "{}", negative.output());
    assert!(negative.output().contains("read -300"), "{}", negative.output());

    // An alignment that underflows is refused, not clamped.
    let underflow = run("-500");
    assert_eq!(underflow.code, 2, "{}", underflow.output());
    assert!(underflow.output().contains("outside the index range"), "{}", underflow.output());

    // The reported offset is the net one.
    let off_by_one = run("301");
    assert_eq!(off_by_one.code, 0, "{}", off_by_one.output());
    assert!(off_by_one.output().contains("offset +1"), "{}", off_by_one.output());
    assert!(off_by_one.output().contains("alignment: 2 of 2 frames paired"), "{}", off_by_one.output());
}
