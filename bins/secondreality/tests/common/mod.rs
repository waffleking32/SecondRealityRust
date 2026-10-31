//! What the binary's integration tests share: the repository root, a headless dump read into
//! memory (one run per argument list, cached), the reference tree's frames, a length-checked
//! differ and the music traces.
//!
//! Each test crate includes this with `mod common;` and uses a subset of it.
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// The repository root: where `assets/` and `reference/` are. A run reads no media from disk
/// (it is embedded); runs start here so relative paths in a test resolve in the checkout.
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Run the binary with the repository root ([`root`]) as its working directory.
pub fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(root())
        .args(args)
        .output()
        .unwrap()
}

/// A scratch directory of this process, empty when made and removed on drop.
pub struct Scratch(PathBuf);

impl Scratch {
    pub fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("sr-it-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The directory as a command-line argument.
    pub fn arg(&self) -> &str {
        self.0.to_str().unwrap()
    }

    pub fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One dumped frame, with the manifest's fields for it.
pub struct Frame {
    pub part: String,
    pub width: u64,
    pub height: u64,
    pub music_position: Option<String>,
    pub rgb: Vec<u8>,
    pub idx: Vec<u8>,
}

/// One line of `--positions`.
pub struct Tick {
    pub pass: u32,
    pub part: String,
    pub position: Option<(u8, u8)>,
}

/// A headless run's dump, held in memory: its directory is gone once it is read.
pub struct Dump {
    pub manifest: serde_json::Value,
    frames: BTreeMap<u32, Frame>,
    /// The `--positions` timeline, when the run was asked for one.
    pub timeline: BTreeMap<u32, Tick>,
}

impl Dump {
    /// Run `args --headless --dump-frames <indices>` and read what it wrote. The run must exit 0,
    /// the manifest must list exactly the indices asked for, and each file must be the size its
    /// geometry says: a hole or a short file would read as a frame that matched.
    pub fn new(args: &[&str], indices: &[u32]) -> Dump {
        Dump::run(args, indices, false)
    }

    /// The same, with the run's `--positions` timeline read too.
    pub fn with_positions(args: &[&str], indices: &[u32]) -> Dump {
        Dump::run(args, indices, true)
    }

    fn run(args: &[&str], indices: &[u32], positions: bool) -> Dump {
        static RUNS: AtomicU32 = AtomicU32::new(0);
        let scratch = Scratch::new(&format!("dump-{}", RUNS.fetch_add(1, Ordering::Relaxed)));
        let mut wanted = indices.to_vec();
        wanted.sort_unstable();
        wanted.dedup();
        let list = wanted.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let frames_dir = scratch.join("frames");
        let positions_file = scratch.join("positions.csv");
        let mut command = Command::new(env!("CARGO_BIN_EXE_secondreality"));
        command
            .current_dir(root())
            .args(args)
            .args(["--headless", "--dump-frames", &list, "--dump-dir"])
            .arg(&frames_dir);
        if positions {
            command.arg("--positions").arg(&positions_file);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&output.stderr));

        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(frames_dir.join("manifest.json")).unwrap(),
        )
        .unwrap();
        let mut frames = BTreeMap::new();
        for entry in manifest["frames"].as_array().unwrap() {
            let index = entry["index"].as_u64().unwrap() as u32;
            let read = |key: &str| {
                let name = entry[key].as_str().unwrap_or_else(|| panic!("frame {index} has no {key}"));
                std::fs::read(frames_dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
            };
            let (width, height) = (entry["width"].as_u64().unwrap(), entry["height"].as_u64().unwrap());
            let frame = Frame {
                part: entry["part"].as_str().unwrap().to_string(),
                width,
                height,
                music_position: entry["music_position"].as_str().map(str::to_string),
                rgb: read("rgb"),
                idx: read("idx"),
            };
            let pixels = (width * height) as usize;
            assert_eq!((frame.rgb.len(), frame.idx.len()), (pixels * 3, pixels), "frame {index}'s files");
            frames.insert(index, frame);
        }
        assert_eq!(frames.keys().copied().collect::<Vec<_>>(), wanted, "{args:?}: the dumped indices");

        let timeline = if positions {
            let text = std::fs::read_to_string(&positions_file).unwrap();
            let mut lines = text.lines();
            assert_eq!(lines.next(), Some("index,pass,part,order,row"), "the CSV's header");
            lines
                .map(|line| {
                    let fields: Vec<&str> = line.split(',').collect();
                    assert_eq!(fields.len(), 5, "{line}");
                    let position = (!fields[3].is_empty())
                        .then(|| (fields[3].parse().unwrap(), fields[4].parse().unwrap()));
                    let tick = Tick { pass: fields[1].parse().unwrap(), part: fields[2].to_string(), position };
                    (fields[0].parse().unwrap(), tick)
                })
                .collect()
        } else {
            BTreeMap::new()
        };
        Dump { manifest, frames, timeline }
    }

    pub fn frame(&self, index: u32) -> &Frame {
        self.frames.get(&index).unwrap_or_else(|| panic!("the dump holds no frame {index}"))
    }

    pub fn rgb(&self, index: u32) -> &[u8] {
        &self.frame(index).rgb
    }

    pub fn idx(&self, index: u32) -> &[u8] {
        &self.frame(index).idx
    }

    pub fn part(&self, index: u32) -> &str {
        &self.frame(index).part
    }

    /// The published `order:row` at `index`; the run must have a player.
    pub fn position(&self, index: u32) -> &str {
        self.frame(index)
            .music_position
            .as_deref()
            .unwrap_or_else(|| panic!("frame {index} has no music position"))
    }

    /// `(index, part, width, height)` of every frame.
    pub fn listing(&self) -> Vec<(u32, &str, u64, u64)> {
        self.frames
            .iter()
            .map(|(index, frame)| (*index, frame.part.as_str(), frame.width, frame.height))
            .collect()
    }
}

/// [`Dump::new`] once per argument list and index list in this test binary: tests that read one
/// run name the same list and share it.
pub fn dump(args: &[&str], indices: &[u32]) -> Arc<Dump> {
    type Cache = Mutex<HashMap<String, Arc<OnceLock<Arc<Dump>>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let key = format!("{args:?} {indices:?}");
    let cell = CACHE.get_or_init(Default::default).lock().unwrap().entry(key).or_default().clone();
    cell.get_or_init(|| Arc::new(Dump::new(args, indices))).clone()
}

/// A filed reference frame: `reference/parts/<capture>/frame_<index>.<extension>`.
pub fn reference(capture: &str, index: u32, extension: &str) -> Vec<u8> {
    let path = root().join(format!("reference/parts/{capture}/frame_{index:06}.{extension}"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// How many `unit`-byte pixels differ. The lengths are asserted first: a short read is not a
/// comparison.
pub fn differing(a: &[u8], b: &[u8], unit: usize) -> usize {
    assert_eq!(a.len(), b.len(), "a short read is not a comparison");
    a.chunks_exact(unit).zip(b.chunks_exact(unit)).filter(|(x, y)| x != y).count()
}

/// A music trace: frame → the published (order, row).
pub type Trace = BTreeMap<u32, (u8, u8)>;

/// `reference/music/<name>/trace.csv`, read once per test binary.
pub fn trace(name: &str) -> Arc<Trace> {
    static TRACES: OnceLock<Mutex<HashMap<String, Arc<Trace>>>> = OnceLock::new();
    let mut traces = TRACES.get_or_init(Default::default).lock().unwrap();
    traces
        .entry(name.to_string())
        .or_insert_with(|| {
            let path = root().join(format!("reference/music/{name}/trace.csv"));
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let mut lines = text.lines();
            let header: Vec<&str> = lines.next().unwrap().split(',').collect();
            let column = |wanted: &str| header.iter().position(|name| *name == wanted).unwrap();
            let (frame, ord, row) = (column("frame"), column("ord"), column("row"));
            Arc::new(
                lines
                    .map(|line| {
                        let fields: Vec<&str> = line.split(',').collect();
                        let number = |at: usize| fields[at].trim().parse::<u32>().unwrap();
                        (number(frame), (number(ord) as u8, number(row) as u8))
                    })
                    .collect(),
            )
        })
        .clone()
}

/// The trace's `order:row` at `frame`, as a manifest spells a position.
pub fn trace_position(name: &str, frame: u32) -> String {
    let (order, row) = *trace(name).get(&frame).unwrap_or_else(|| panic!("{name} holds no frame {frame}"));
    format!("{order}:{row}")
}
