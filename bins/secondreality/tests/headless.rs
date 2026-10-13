use std::path::PathBuf;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality")).args(args).output().unwrap()
}

/// A headless run of the ported block 5, from the repository root.
///
/// The root, and not wherever `cargo test` happens to be, because the ported
/// parts read their assets relative to the working directory: the test card
/// needs none, a block run needs all of them. The run must exit 0 — every
/// requested index is produced, and a missing one is a failure with a code
/// rather than a hole in a dump that reads as a frame that matched.
fn run_block5(indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-headless-block5-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(root)
        .args(["--block", "5", "--headless", "--dump-frames", indices, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

#[test]
fn headless_writes_two_frames_and_a_manifest() {
    let dir = std::env::temp_dir().join(format!("sr-headless-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = run(&["--headless", "--part", "testcard", "--frames", "2",
                    "--dump-dir", dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["frames"].as_array().unwrap().len(), 2);

    let idx = std::fs::read(dir.join("testcard_000000.idx")).unwrap();
    assert_eq!(idx.len(), 320 * 200);
    let rgb = std::fs::read(dir.join("testcard_000000.rgb")).unwrap();
    assert_eq!(rgb.len(), 320 * 200 * 3);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_run_plays_its_parts_in_order_on_one_reference_numbering() {
    // `SECOND 5` end to end, through the real binary: the whole ported block,
    // in the demo's order, on the one counter the shell keeps. This is the
    // binary's own claim — the same `Sequence` the window drives — and the run
    // is the whole of it, not a part's own start.
    //
    // **The indices are consecutive at both hand-overs**, which is what makes
    // this a continuity check and not a list of numbers: a run of `--block 5`
    // ticks one index per tick, so 1211 and 1212 are both produced only if
    // ENDLOGO's end and CRED's start are adjacent — and a counter that jumped
    // would leave one of them missing, which is an error and not a dump.
    let (dir, manifest) = run_block5("1211,1212,8834,8835");

    // The numbering is the reference's: a run of a block begins at the boot's
    // first graphics frame, which is what makes these indices comparable with
    // `reference/`.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "endlogo", "the run's first frame is ENDLOGO's");

    let listed: Vec<(u64, &str)> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| {
            (frame["index"].as_u64().unwrap(), frame["part"].as_str().unwrap())
        })
        .collect();
    // Each index is filed under the part that drew it, and the parts come in
    // the demo's own order — ENDLOGO, then CRED (`MAIN/U2.ASM:910-917`), then
    // ENDSCRL. 1211/1212 and 8834/8835 are the two hand-overs the chain has.
    assert_eq!(
        listed,
        [(1211, "endlogo"), (1212, "cred"), (8834, "cred"), (8835, "endscrl")]
    );

    // Every requested frame is on disk, at the geometry the manifest claims —
    // ENDLOGO's double-scan is 320x400 and CRED's and ENDSCRL's 320x200.
    for frame in manifest["frames"].as_array().unwrap() {
        let index = frame["index"].as_u64().unwrap();
        let part = frame["part"].as_str().unwrap();
        let width = frame["width"].as_u64().unwrap();
        let height = frame["height"].as_u64().unwrap();
        assert!(
            dir.join(format!("{part}_{index:06}.rgb")).is_file(),
            "{part} {index} has no .rgb"
        );
        assert!(
            dir.join(format!("{part}_{index:06}.idx")).is_file(),
            "{part} {index} has no .idx"
        );
        assert_eq!(
            std::fs::metadata(dir.join(format!("{part}_{index:06}.rgb"))).unwrap().len(),
            width * height * 3,
            "{part} {index} is not the geometry the manifest claims"
        );
        assert_eq!(
            std::fs::metadata(dir.join(format!("{part}_{index:06}.idx"))).unwrap().len(),
            width * height,
            "{part} {index} is not the geometry the manifest claims"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}
