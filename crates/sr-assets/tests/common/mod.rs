//! Helpers shared by the sr-assets integration tests. Tests run in the crate's directory;
//! `assets/` holds the curated copies, verified byte for byte against
//! `upstream/SecondReality` (commit `071a82e`) by `cargo xtask assets verify --tree`.

// Each test binary uses its own subset.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub const ASSETS: &str = "../../assets";
pub const UPSTREAM: &str = "../../upstream/SecondReality";

/// The curated file at `assets/<rel>`.
pub fn asset_path(rel: &str) -> PathBuf {
    Path::new(ASSETS).join(rel)
}

pub fn asset(rel: &str) -> Vec<u8> {
    read(&asset_path(rel))
}

/// The vendored original's `<rel>`. Read-only.
pub fn upstream(rel: &str) -> Vec<u8> {
    read(&Path::new(UPSTREAM).join(rel))
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Every file under `assets/` whose extension is one of `extensions` (any case), sorted.
pub fn assets_with(extensions: &[&str]) -> Vec<PathBuf> {
    fn walk(dir: &Path, extensions: &[&str], out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display())) {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                walk(&path, extensions, out);
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                if extensions.iter().any(|k| ext.eq_ignore_ascii_case(k)) {
                    out.push(path);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(Path::new(ASSETS), extensions, &mut out);
    out.sort();
    out
}

/// FNV-1a, 64-bit: a regression pin for a decoded image.
pub fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &x| (h ^ x as u64).wrapping_mul(0x100_0000_01b3))
}

/// An IFF file's chunk `id`: the offset of its data and the data, walked here rather than by the
/// decoder, so a wrong walk in one cannot hide behind the other.
pub fn iff_chunk<'a>(file: &'a [u8], id: &[u8; 4]) -> (usize, &'a [u8]) {
    let mut at = 12;
    while at + 8 <= file.len() {
        let size = u32::from_be_bytes([file[at + 4], file[at + 5], file[at + 6], file[at + 7]]) as usize;
        if &file[at..at + 4] == id {
            return (at + 8, &file[at + 8..at + 8 + size]);
        }
        at += 8 + size + (size & 1);
    }
    panic!("no {} chunk in this file", String::from_utf8_lossy(id));
}
