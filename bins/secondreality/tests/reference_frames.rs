//! Every filed reference frame against the count `cargo xtask rediff` registers for it, as a
//! cargo test.
//!
//! The table is rediff's own, `xtask/src/rediff/table.rs`, included here by path. Its entries
//! are grouped by their run arguments, and each group is one headless run of the binary that
//! dumps the union of the group's rendered indices. Every frame is then compared the way
//! rediff compares it: the capture's frame `reference` against the dump's frame `rendered`,
//! exactly (no ±1 search), in the plane the row names (`.idx` bytes, or `.rgb` pixels with any
//! channel differing), and its differing count must equal `registered`.
//!
//! The one group that runs block 1 (`fullrun/*`, `u2a`, `u2a-speed`) is release-only, like
//! `fullrun.rs`; every other group runs in both profiles (the slowest, block 5, takes about
//! 40 s in debug):
//!
//! ```text
//! cargo test --release -p secondreality --test reference_frames
//! ```
//!
//! What rediff adds on top (the table against the tree, the report) is `xtask/tests/rediff.rs`
//! and `cargo xtask rediff` itself.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../../../xtask/src/rediff/table.rs"]
mod table;

use table::{Entry, Frame, Plane, TABLE};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One frame of a manifest: its files and its geometry (a dump's frame carries its own; a
/// capture's falls back to the manifest's).
struct Filed {
    idx: Option<String>,
    rgb: String,
    geometry: (u64, u64),
}

/// The frame `index` of the manifest in `dir`.
fn filed(dir: &Path, index: u32) -> Filed {
    let path = dir.join("manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display())),
    )
    .unwrap();
    let frame = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"].as_u64() == Some(u64::from(index)))
        .unwrap_or_else(|| panic!("{} holds no frame {index}", path.display()));
    let dimension = |key: &str| frame[key].as_u64().or(manifest[key].as_u64()).unwrap();
    Filed {
        idx: frame["idx"].as_str().map(str::to_string),
        rgb: frame["rgb"].as_str().unwrap().to_string(),
        geometry: (dimension("width"), dimension("height")),
    }
}

fn read(dir: &Path, file: &str) -> Vec<u8> {
    let path = dir.join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The differing count of one row, as rediff's `compare_frame` computes it.
fn differing(capture: &Path, dump: &Path, frame: &Frame) -> u32 {
    let reference = filed(capture, frame.reference);
    let rendered = filed(dump, frame.rendered);
    assert_eq!(
        reference.geometry, rendered.geometry,
        "reference {} against rendered {}: a geometry mismatch is never a count",
        frame.reference, frame.rendered
    );
    let pixels = (reference.geometry.0 * reference.geometry.1) as usize;
    let (stride, a, b) = match frame.plane {
        Plane::Indexed => {
            let file = |filed: &Filed, side: &str| {
                filed.idx.clone().unwrap_or_else(|| panic!("the {side} has no .idx"))
            };
            (1, read(capture, &file(&reference, "capture")), read(dump, &file(&rendered, "dump")))
        }
        Plane::Rgb => (3, read(capture, &reference.rgb), read(dump, &rendered.rgb)),
    };
    assert_eq!((a.len(), b.len()), (pixels * stride, pixels * stride), "a short read is not a comparison");
    a.chunks_exact(stride).zip(b.chunks_exact(stride)).filter(|(x, y)| x != y).count() as u32
}

/// Run the binary once with `args` and check every frame of every entry run with them.
fn check(args: &[&str]) {
    let entries: Vec<&Entry> = TABLE.iter().filter(|entry| entry.args == args).collect();
    assert!(!entries.is_empty(), "no table entry runs with {args:?}");
    let indices: BTreeSet<u32> =
        entries.iter().flat_map(|entry| entry.frames).map(|frame| frame.rendered).collect();
    let list = indices.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let dump = std::env::temp_dir()
        .join(format!("sr-reference-frames-{}-{}", std::process::id(), args.join("_")));
    let _ = std::fs::remove_dir_all(&dump);
    std::fs::create_dir_all(&dump).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(args)
        .args(["--headless", "--dump-frames", &list, "--dump-dir"])
        .arg(&dump)
        .output()
        .unwrap();
    // Compare, then remove the dump whatever the comparison found.
    let compared = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let mut off = Vec::new();
        for entry in &entries {
            let capture = repository_root().join("reference/parts").join(entry.capture);
            for frame in entry.frames {
                let count = differing(&capture, &dump, frame);
                if count != frame.registered {
                    off.push(format!(
                        "{} {} at {} ({:?}): {count} differing, {} registered ({})",
                        entry.capture,
                        frame.reference,
                        frame.rendered,
                        frame.plane,
                        frame.registered,
                        frame.register
                    ));
                }
            }
        }
        assert!(off.is_empty(), "off their registered count:\n{}", off.join("\n"));
    }));
    let _ = std::fs::remove_dir_all(&dump);
    compared.unwrap_or_else(|panic| std::panic::resume_unwind(panic));
}

/// One test a run: `name => [args]`, each optionally release-only.
macro_rules! groups {
    ($($(#[$attr:meta])* $name:ident => [$($arg:literal),*];)*) => {
        /// Every group's arguments, so that an entry no test runs is a failure.
        const GROUPS: &[&[&str]] = &[$(&[$($arg),*]),*];
        $(
            #[test]
            $(#[$attr])*
            fn $name() {
                check(&[$($arg),*]);
            }
        )*
    };
}

groups! {
    part_3dsinfld => ["--part", "3dsinfld"];
    part_alku => ["--part", "alku"];
    part_beglogo => ["--part", "beglogo"];
    part_ddstars => ["--part", "ddstars"];
    part_endlogo => ["--part", "endlogo"];
    part_grid => ["--part", "grid"];
    part_hard => ["--part", "hard"];
    part_jplogo => ["--part", "jplogo"];
    part_lnszoom => ["--part", "lnszoom"];
    part_minvball => ["--part", "minvball"];
    part_mntscrl => ["--part", "mntscrl"];
    part_pam => ["--part", "pam"];
    part_panicend => ["--part", "panicend"];
    part_plzpart => ["--part", "plzpart"];
    part_rayscrl => ["--part", "rayscrl"];
    part_techno => ["--part", "techno"];
    part_tunneli => ["--part", "tunneli"];
    part_twist => ["--part", "twist"];
    #[cfg_attr(debug_assertions, ignore = "release only: a 46,000-tick run; cargo test --release -p secondreality --test reference_frames")]
    block_1 => ["--block", "1"];
    block_2 => ["--block", "2"];
    block_4 => ["--block", "4"];
    block_5 => ["--block", "5"];
    block_5_loop => ["--block", "5", "--loop"];
}

#[test]
fn every_table_entry_is_in_a_group() {
    for entry in TABLE {
        assert!(GROUPS.contains(&entry.args), "{}: no test runs {:?}", entry.capture, entry.args);
    }
}
