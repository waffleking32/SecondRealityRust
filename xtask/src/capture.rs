//! A capture on disk: a directory holding `manifest.json` and the frame files it names, and the
//! walk that finds every capture below a tree. `compare`, `rediff`, `reference verify` and
//! `frames sheet` all read captures through this module, so they agree on what a capture is.
//!
//! Two manifest shapes meet here: the capture manifest `reference capture` writes
//! (`reference.rs`) and the dump manifest `secondreality --headless --dump-dir` writes. They
//! share `version`, `width`, `height` and `frames: [{index, idx, rgb}]`; the rest is ignored.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::reference::MANIFEST_VERSION;

/// The file a capture directory is described by, on both sides.
pub(crate) const MANIFEST_FILE: &str = "manifest.json";

/// The fields both manifests share.
#[derive(Debug, Deserialize)]
pub(crate) struct Manifest {
    pub(crate) version: u32,
    pub(crate) width: u16,
    pub(crate) height: u16,
    pub(crate) frames: Vec<FrameRef>,
}

/// One frame on either side. `idx` is absent when that side wrote no indices (a capture whose
/// coherence check failed, D15); the port's dump carries each frame's own geometry, because a
/// part can change the view mid-run.
#[derive(Debug, Deserialize)]
pub(crate) struct FrameRef {
    pub(crate) index: u32,
    #[serde(default)]
    pub(crate) idx: Option<String>,
    pub(crate) rgb: String,
    #[serde(default)]
    pub(crate) width: Option<u16>,
    #[serde(default)]
    pub(crate) height: Option<u16>,
}

impl FrameRef {
    /// This frame's geometry: its own when it carries one, the manifest's otherwise.
    pub(crate) fn geometry(&self, manifest: &Manifest) -> (u16, u16) {
        (
            self.width.unwrap_or(manifest.width),
            self.height.unwrap_or(manifest.height),
        )
    }
}

impl Manifest {
    /// The first frame carrying `index`.
    pub(crate) fn frame(&self, index: u32) -> Option<&FrameRef> {
        self.frames.iter().find(|frame| frame.index == index)
    }
}

/// A capture directory the walk found.
pub(crate) struct CaptureDir {
    /// The directory's path below the tree's root, `/`-separated (`plzpart/plasma`); empty for
    /// the root itself.
    pub name: String,
    pub dir: PathBuf,
}

/// Every capture `dir` holds, at any depth, sorted by name. A directory with a `manifest.json`
/// is a capture, and the walk does not stop there: a manifest below a capture is a capture
/// too. Measuring fewer captures than the tree holds is how a green run lies, so nothing below
/// the root is skipped. A tree with none is an error.
pub(crate) fn capture_directories(dir: &Path) -> Result<Vec<CaptureDir>, String> {
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    let mut captures = Vec::new();
    collect(dir, dir, &mut captures)?;
    captures.sort_by(|a, b| a.name.cmp(&b.name));
    if captures.is_empty() {
        return Err(format!(
            "{} holds no {MANIFEST_FILE} (looked in it and in every subdirectory)",
            dir.display()
        ));
    }
    Ok(captures)
}

fn collect(root: &Path, dir: &Path, captures: &mut Vec<CaptureDir>) -> Result<(), String> {
    if dir.join(MANIFEST_FILE).is_file() {
        captures.push(CaptureDir {
            name: capture_name(root, dir),
            dir: dir.to_path_buf(),
        });
    }
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut subdirectories = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{}: {e}", dir.display()))?.path();
        if path.is_dir() {
            subdirectories.push(path);
        }
    }
    subdirectories.sort();
    for subdirectory in &subdirectories {
        collect(root, subdirectory, captures)?;
    }
    Ok(())
}

/// A capture's path below `root`, `/`-separated on every platform so names from two trees
/// can be matched.
fn capture_name(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .unwrap_or(Path::new(""))
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// The manifest in `dir`, refused when its version is not the one this tool reads.
pub(crate) fn read_manifest(dir: &Path) -> Result<Manifest, String> {
    let path = dir.join(MANIFEST_FILE);
    let json = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let manifest: Manifest =
        serde_json::from_str(&json).map_err(|e| format!("{}: {e}", path.display()))?;
    if manifest.version != MANIFEST_VERSION {
        return Err(format!(
            "{} says version {}; this tool reads version {MANIFEST_VERSION}",
            path.display(),
            manifest.version
        ));
    }
    Ok(manifest)
}

/// The bytes of a file a manifest names.
pub(crate) fn read_frame(dir: &Path, file: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(file);
    fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}
