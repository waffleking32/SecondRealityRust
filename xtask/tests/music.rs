//! `cargo xtask music` end to end, and the music trace as a `reference verify` capture: the
//! symbol map behind `docs/fidelity/music-clock.md`'s addresses comes from a command, a flipped
//! byte of a trace is a finding with both digests in it, and a `kind` the verifier does not know
//! is refused by name. The commands that drive the emulator are not run here; what they refuse
//! before touching it is.

mod common;

use std::fs;

use common::{fixture, repo_root, scratch, xtask};

const OBJECT: &str = "upstream/SecondReality/MAIN/STMIK.300";

// -------------------------------------------------------------------------------------
// The symbol map
// -------------------------------------------------------------------------------------

/// The map the trace reads through, printed by a command: the census, the one
/// segment, and the seventeen words the trace reads at their measured offsets.
#[test]
fn the_symbols_command_prints_the_map() {
    let run = xtask(&["music", "symbols", "--object", OBJECT]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    for expected in [
        "24849 bytes, 278 record(s)",
        "0x80 THEADR    1 record(s)",
        "0x88 COMENT    5 record(s)",
        "0x8A MODEND    1 record(s)",
        "0x90 PUBDEF    45 record(s)",
        "0x96 LNAMES    2 record(s)",
        "0x98 SEGDEF    1 record(s)",
        "0xA0 LEDATA    105 record(s)",
        "0xA2 LIDATA    118 record(s)",
        "29607 bytes",
        "0x08F7   2295 _np_zinfo",
        "0x08FD   2301 _np_row",
        "0x0903   2307 _np_loop",
        "0x2002   8194 _np_speed",
        "0x090F   2319 _np_tempo",
        "needle",
    ] {
        assert!(text.contains(expected), "{expected} is not in:\n{text}");
    }
}

/// A symbol map cannot be read from an object that is not one, and the refusal
/// says which file.
#[test]
fn a_file_that_is_not_an_object_is_refused_by_name() {
    let run = xtask(&["music", "symbols", "--object", "assets/MAIN/MUSIC0.S3M"]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("MUSIC0.S3M"), "{}", run.output());
}

// -------------------------------------------------------------------------------------
// What `music trace` refuses before it starts anything
// -------------------------------------------------------------------------------------

/// `reference/` is input-only: the one place inside it a trace may be written is
/// a new directory under `reference/music/`, and everything else is refused
/// **by name** — including a frame capture's own directory, and
/// `reference/music` itself.
#[test]
fn a_trace_may_not_be_written_among_the_reference() {
    for out in [
        "reference/parts/beglogo",
        "reference/check_a",
        "reference/original",
        "reference/music",
    ] {
        let run = xtask(&[
            "music", "trace", "--cycles", "30000", "--frames", "0..10", "--out", out,
        ]);
        assert_eq!(run.code, 2, "{out}: {}", run.output());
        let text = run.output();
        assert!(
            text.contains("reference/") && text.contains("reference/music"),
            "{out} is refused by name: {text}"
        );
    }
    // And a frame capture that holds a manifest is refused for what it is.
    let out = fixture("verify-capture");
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(
        run.output().contains("frame capture"),
        "{}",
        run.output()
    );
}

/// A trace with no pinned cycles is not reproducible and is refused, and a
/// window that is empty or has a hole in it is not a window.
#[test]
fn a_trace_needs_a_pinned_clock_and_one_window() {
    let run = xtask(&["music", "trace", "--cycles", "0", "--frames", "0..10"]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("cycles"), "{}", run.output());

    for frames in ["", "10..10", "10..0", "0,2", "0..5,9..12"] {
        let run = xtask(&[
            "music",
            "trace",
            "--cycles",
            "30000",
            "--frames",
            frames,
            "--out",
            "out/should-not-be-written",
        ]);
        assert_eq!(run.code, 2, "--frames {frames:?}: {}", run.output());
    }
}

/// The keys are part of the measurement — the card they select is in the
/// manifest — so a name the emulator's mapper does not know is refused before
/// anything starts (D17).
#[test]
fn a_key_that_answers_nothing_is_refused() {
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--keys",
        "KBD_kp_8",
        "--out",
        "out/should-not-be-written",
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("KBD_kp_8"), "{}", run.output());
}

/// A directory that already holds a trace is not written into without
/// `--force`.
#[test]
fn an_existing_trace_is_not_replaced_without_force() {
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--out",
        "reference/music/second5",
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("--force"), "{}", run.output());
}

// -------------------------------------------------------------------------------------
// The trace as a reference, and the verifier that has to see it
// -------------------------------------------------------------------------------------

/// **The test that matters most**: a copy of a trace with one byte flipped is a
/// finding — exit 1, naming the file and both digests. A verifier that has never
/// been seen to fail is worth nothing.
#[test]
fn a_flipped_byte_of_a_trace_fails_the_verifier() {
    let tree = scratch("flipped");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    for file in ["trace.csv", "manifest.json"] {
        fs::copy(
            repo_root().join("reference/music/second5").join(file),
            capture.join(file),
        )
        .unwrap();
    }
    let path = capture.join("trace.csv");
    let mut bytes = fs::read(&path).unwrap();
    // A flip that stays valid UTF-8, so the digest is what catches it.
    let at = bytes.iter().position(|byte| *byte == b'4').unwrap();
    bytes[at] = b'5';
    fs::write(&path, &bytes).unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 1, "{}", run.output());
    let text = run.output();
    assert!(text.contains("trace.csv"), "{text}");
    assert!(text.contains("sha256"), "{text}");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(capture.join("manifest.json")).unwrap()).unwrap();
    let recorded = manifest["trace"]["sha256"].as_str().unwrap();
    assert!(
        text.contains(recorded),
        "the digest the manifest records is in the finding: {text}"
    );
}

/// A `kind` the tool does not know is a **failure and not a skip**: a verifier
/// that walked past it would report a green run over a capture it never read.
#[test]
fn a_capture_of_an_unknown_kind_is_refused_by_name() {
    let tree = scratch("unknown-kind");
    let capture = tree.join("mystery");
    fs::create_dir_all(&capture).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("reference/music/second5/manifest.json")).unwrap(),
    )
    .unwrap();
    manifest["kind"] = serde_json::json!("video-trace");
    fs::write(
        capture.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 2, "{}", run.output());
    let text = run.output();
    assert!(text.contains("video-trace"), "the kind is named: {text}");
    assert!(text.contains("does not know") || text.contains("refused"), "{text}");
}

/// A **crafted** window is a finding and never a crash: `[0, u64::MAX]` has no
/// span a `u64` can hold, so the recomputed coverage is the empty one and the
/// manifest's claim is reported — the verifier keeps doing its job on a
/// manifest that is lying to it.
#[test]
fn a_window_whose_span_overflows_is_a_finding_not_a_panic() {
    let tree = scratch("overflow-window");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    fs::copy(
        repo_root().join("reference/music/second5/trace.csv"),
        capture.join("trace.csv"),
    )
    .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("reference/music/second5/manifest.json")).unwrap(),
    )
    .unwrap();
    manifest["coverage"]["window"] = serde_json::json!([0, u64::MAX]);
    fs::write(
        capture.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 1, "a finding, not a crash: {}", run.output());
    let text = run.output();
    // The finding names the fields the window cannot support — the manifest and
    // the file agree about the *window*, so what disagrees is what the window
    // was claimed to imply.
    assert!(text.contains("does not match the file"), "{text}");
    assert!(text.contains("requested:"), "the field is named: {text}");
    assert!(text.contains("missing:"), "the other field is named: {text}");
}

/// A trace whose rows do not match the coverage its manifest claims is a
/// finding: the coverage is recomputed from the file's own frames rather than
/// believed.
#[test]
fn a_coverage_claim_the_file_does_not_support_is_a_finding() {
    let tree = scratch("coverage");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    fs::copy(
        repo_root().join("reference/music/second5/trace.csv"),
        capture.join("trace.csv"),
    )
    .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("reference/music/second5/manifest.json")).unwrap(),
    )
    .unwrap();
    // Claim a window the rows do not cover, and say it is per-frame.
    manifest["coverage"]["window"] = serde_json::json!([0, 2000]);
    manifest["coverage"]["per_frame"] = serde_json::json!(true);
    fs::write(
        capture.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 1, "{}", run.output());
    let text = run.output();
    assert!(
        text.contains("coverage") || text.contains("per-frame"),
        "{text}"
    );
    // The trace file itself still hashes as recorded, so the only finding is
    // the coverage claim — which is the point: the digest is not the only check.
    assert!(!text.contains("sha256"), "{text}");
}

// -------------------------------------------------------------------------------------
// The analysis, which is where the note's numbers come from
// -------------------------------------------------------------------------------------

/// `music analyse` on `second5`: the note's sections and numbers, and the pre-sampling boundary
/// named.
#[test]
fn the_analysis_prints_the_notes_numbers() {
    let run = xtask(&["music", "analyse", "--trace", "reference/music/second5"]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    for section in [
        "1. where the music starts",
        "2. the tick rate",
        "3. the quantisation",
        "4. the soundcard and mixer, from the guest",
        "5. zplus and zinfo",
        "6. the order list, and the 254/255 entries",
    ] {
        assert!(text.contains(section), "{section} is not in:\n{text}");
    }
    assert!(text.contains("first advancing frame: 410"), "{text}");
    assert!(text.contains("_zoutputmode     5"), "{text}");
    assert!(text.contains("_zmixspeed       20000"), "{text}");
    assert!(
        text.contains("order 27 (254): never held — the counter went from ord 26 to ord 28"),
        "{text}"
    );
    assert!(
        text.contains("147")
            && text.contains("the boundary: the segment was located after they had gone"),
        "the boundary is named:\n{text}"
    );
}

/// Over every filed trace (the tree's walk, so a trace filed later is covered), one `analyse`
/// each: the music's start is per recipe (`music-clock.md` §2). A trace whose window holds the start gives the
/// recipe's anchor, the first row change back-extrapolated at the measured frames a tick, and
/// the sessions of one recipe agree to 0.5 frames; the `zframe` column's first non-zero frame
/// sits within the recipe's measured gap of it. A trace from a later window extrapolates the
/// last tick-counter reset instead, which only has to precede its window.
#[test]
fn the_anchor_is_the_same_in_every_filed_session() {
    /// The frame the music begins on, per recipe: `SECOND 1`'s initial `_zplaysong(0)` (no
    /// restart, `MAIN/U2.ASM:766-775`), `SECOND 2`'s boot by the rows, and the restart recipes.
    const ANCHORS: &[(&str, f64)] = &[("1", 399.6), ("2", 401.1), ("3", 408.0), ("5", 408.0), ("U", 408.0)];
    /// How far the `zframe` column's first non-zero frame may sit from the anchor: `second1`'s
    /// first sample caught two ticks, `second2b`'s boot ticks come at nearly one a frame.
    const DIRECT_GAP: &[(&str, f64)] = &[("1", 5.4), ("2", 6.9), ("3", 2.9), ("5", 2.9), ("U", 2.9)];
    const MUSIC_START_FRAME: i64 = 408;

    let lookup = |table: &[(&str, f64)], recipe: &str| -> f64 {
        *table
            .iter()
            .find(|(key, _)| *key == recipe)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("no measured value for recipe {recipe:?}"))
    };
    let number_after = |text: &str, marker: &str| -> Option<f64> {
        text.lines()
            .find(|line| line.contains(marker))
            .and_then(|line| line.split(marker).nth(1))
            .and_then(|rest| rest.split(' ').next())
            .and_then(|number| number.parse().ok())
    };

    let mut names: Vec<String> = fs::read_dir(repo_root().join("reference/music"))
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.unwrap();
            entry
                .path()
                .join("manifest.json")
                .is_file()
                .then(|| entry.file_name().to_string_lossy().into_owned())
        })
        .collect();
    names.sort();
    assert_eq!(names.len(), 18, "the filed traces: {names:?}");

    // recipe -> (trace, anchor, direct) for the traces that hold the start.
    let mut boots: std::collections::BTreeMap<String, Vec<(String, f64, f64)>> =
        std::collections::BTreeMap::new();
    let mut windows = Vec::new();
    for trace in &names {
        let path = format!("reference/music/{trace}");
        let run = xtask(&["music", "analyse", "--trace", &path]);
        assert_eq!(run.code, 0, "{trace}: {}", run.output());
        let text = run.output();

        let anchor = number_after(&text, "so that row began at frame ")
            .unwrap_or_else(|| panic!("{trace}: no anchor line in\n{text}"));
        let direct = number_after(&text, "the `zframe` column shows: frame ")
            .unwrap_or_else(|| panic!("cannot read the zframe anchor from {trace}"));

        let manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(repo_root().join(&path).join("manifest.json")).unwrap(),
        )
        .unwrap();
        let first = manifest["coverage"]["window"][0].as_i64().unwrap();
        let recipe = manifest["arguments"]
            .as_array()
            .expect("a trace records its arguments")
            .iter()
            .map(|value| value.as_str().expect("an argument is a string"))
            .collect::<Vec<_>>()
            .join(",");

        if first <= MUSIC_START_FRAME {
            let bound = lookup(DIRECT_GAP, &recipe);
            assert!(
                (direct - anchor).abs() <= bound + 1e-9,
                "{trace}: the two readings are {} frames apart ({anchor} and {direct}), past the \
                 recipe's measured {bound}",
                (direct - anchor).abs()
            );
            boots.entry(recipe).or_default().push((trace.clone(), anchor, direct));
        } else {
            assert!(
                anchor <= first as f64,
                "{trace}: the derived anchor {anchor} postdates the window that produced it \
                 (frame {first})"
            );
            windows.push((trace.clone(), recipe, anchor));
        }
    }
    assert!(!boots.is_empty(), "no filed trace holds the music's start");

    for (recipe, sessions) in &boots {
        let anchors: Vec<f64> = sessions.iter().map(|(_, anchor, _)| *anchor).collect();
        let spread = anchors.iter().cloned().fold(f64::NEG_INFINITY, f64::max)
            - anchors.iter().cloned().fold(f64::INFINITY, f64::min);
        assert!(spread <= 0.5, "recipe {recipe}'s sessions {sessions:?} — spread {spread:.2} frames");
        let measured = lookup(ANCHORS, recipe);
        for (trace, anchor, _) in sessions {
            assert!(
                (anchor - measured).abs() <= 0.5,
                "{trace}: the recipe's anchor is {measured}, and {anchor} is not it: {sessions:?}"
            );
        }
    }
    println!("boot anchors: {boots:?}\nwindow extrapolations (not boot anchors): {windows:?}");
}

/// The analysis checks the trace against the digest its manifest records before
/// reading a number out of it: a trace that does not hash as recorded is not
/// analysed, because every number would be about bytes nobody filed.
#[test]
fn the_analysis_refuses_a_trace_that_does_not_hash() {
    let tree = scratch("analysis-digest");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    for file in ["trace.csv", "manifest.json"] {
        fs::copy(
            repo_root().join("reference/music/second5").join(file),
            capture.join(file),
        )
        .unwrap();
    }
    let path = capture.join("trace.csv");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, &bytes).unwrap();

    let run = xtask(&[
        "music",
        "analyse",
        "--trace",
        capture.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("sha256"), "{}", run.output());
}
