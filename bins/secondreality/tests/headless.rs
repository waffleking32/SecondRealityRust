use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality")).args(args).output().unwrap()
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
