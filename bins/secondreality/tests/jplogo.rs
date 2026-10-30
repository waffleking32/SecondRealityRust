//! JPLOGO through the real binary: the registry, the run's own numbering, the
//! music at the sweep and the keyframes, and D19's holds.
//!
//! The keyframes are rediff's `jplogo` entry, 17700's 51,445 pixels included
//! (a filing no census session holds, A27). The part has no boot anchor: a run
//! numbers its own frames from zero, and the reference is reached by the
//! mapping boot = 17065 + tick (`docs/fidelity/jplogo.md` §3).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/JPLOGO/...` path resolves.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-jplogo-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--part", "jplogo", "--headless", "--dump-frames", indices, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

fn frame_file(dir: &Path, index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(dir.join(format!("jplogo_{index:06}.{extension}"))).unwrap()
}

/// The trace's `order:row` at a boot frame, as the manifest spells it.
fn trace_position(frame: u32) -> String {
    let text =
        std::fs::read_to_string(repository_root().join("reference/music/second3d/trace.csv"))
            .unwrap();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0].parse::<u32>().unwrap() == frame {
            return format!("{}:{}", fields[1], fields[2]);
        }
    }
    panic!("the trace has no sample at {frame}");
}

/// The three keyframes and the tick each is drawn on (see the module comment).
const KEYFRAMES: [(u32, u32); 3] = [(17300, 235), (17500, 435), (17700, 635)];

/// The tick the reference's start window's `17100` sweep frame matches: the
/// entry's step 34, start address 309.
const SWEEP_TICK: u32 = 35;

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_measured_music_row() {
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "jplogo");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 400);
    // The `second3d` trace holds MUSIC1 order 96 across the part's whole
    // start window (17000..17100); the `254` at 97 is crossed at 17109.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 96);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|frame| frame["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [0, 1]);
    assert_eq!(frame_file(&dir, 0, "rgb").len(), 320 * 400 * 3);
    assert_eq!(manifest["frames"][0]["music_position"], "96:58");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The entry's place in the reference's numbering: `17100` is the sweep's
/// start address 309, which the model puts at tick 35, and the trace's
/// position there is `96:62`.
#[test]
fn the_sweeps_17100_is_entry_step_thirty_four() {
    let (dir, manifest) = dump("sweep", &SWEEP_TICK.to_string());
    assert_eq!(manifest["frames"][0]["index"], SWEEP_TICK);
    assert_eq!(manifest["frames"][0]["part"], "jplogo");
    assert_eq!(manifest["frames"][0]["music_position"], trace_position(17100));
    assert_eq!(manifest["frames"][0]["music_position"], "96:62");
    assert_eq!(frame_file(&dir, SWEEP_TICK, "rgb").len(), 320 * 400 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D19's support: 17300 and 17500 hold on the next tick and the tick before
/// each is a different state; 17700's settled state is static. The published
/// position at each keyframe is the trace's.
#[test]
fn the_keyframes_hold_as_the_reference_does_and_publish_the_traces_music() {
    let ticks: Vec<u32> = KEYFRAMES
        .iter()
        .flat_map(|(_, tick)| [tick - 1, *tick, tick + 1])
        .collect();
    let indices = ticks.iter().map(|tick| tick.to_string()).collect::<Vec<_>>().join(",");
    let (dir, manifest) = dump("keyframes", &indices);
    let frame = |tick: u32| frame_file(&dir, tick, "rgb");

    for (index, tick) in [(17300u32, 235u32), (17500, 435)] {
        assert_eq!(frame(tick + 1), frame(tick), "{index}: the held tick");
        assert_ne!(frame(tick - 1), frame(tick), "{index}: the tick before");
    }
    assert_eq!(frame(634), frame(635), "17700: the settled state");
    assert_eq!(frame(636), frame(635), "17700: the settled state");

    for (index, tick) in KEYFRAMES {
        let entry = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["index"].as_u64() == Some(tick as u64))
            .unwrap();
        assert_eq!(
            entry["music_position"],
            serde_json::Value::String(trace_position(index)),
            "frame {index} (tick {tick}): the trace's position"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
