//! Embeds the repository's `assets/` in the binary, so a run needs no files beside it.
//!
//! Writes `$OUT_DIR/assets.rs`: `FILES`, every file under `assets/` (except
//! `PROVENANCE.md`, the tree's documentation) as `(repo-relative path, include_bytes!)`,
//! sorted by path so `src/assets.rs` can binary-search it. Paths use forward slashes on
//! every host. Every directory and file is a `rerun-if-changed`, so adding, removing or
//! changing an asset rebuilds the binary.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The tree's documentation, parsed by `xtask assets`; not media.
const SKIPPED: &str = "assets/PROVENANCE.md";

fn main() {
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let root = manifest.join("../..");
    let assets = root.join("assets");
    println!("cargo:rerun-if-changed={}", assets.display());

    let mut files = Vec::new();
    walk(&assets, "assets", &mut files);
    files.retain(|(rel, _)| rel != SKIPPED);
    files.sort();

    let mut out = String::from(
        "/// Every embedded asset as (repository-relative path, bytes), sorted by path.\n\
         static FILES: &[(&str, &[u8])] = &[\n",
    );
    for (rel, path) in &files {
        let absolute = path
            .canonicalize()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let absolute = absolute
            .to_str()
            .unwrap_or_else(|| panic!("{}: not UTF-8", path.display()));
        // `{:?}` writes a Rust string literal, backslashes and quotes escaped.
        writeln!(out, "    ({rel:?}, include_bytes!({absolute:?})),").unwrap();
    }
    out.push_str("];\n");

    let dest = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("assets.rs");
    std::fs::write(&dest, out).unwrap_or_else(|error| panic!("{}: {error}", dest.display()));
}

/// Every file below `dir`, whose repository-relative name is `rel`, as (`rel/...`, path).
fn walk(dir: &Path, rel: &str, files: &mut Vec<(String, PathBuf)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    let mut entries: Vec<_> = entries
        .map(|entry| entry.expect("a directory entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let name = name
            .to_str()
            .unwrap_or_else(|| panic!("{}: not UTF-8", path.display()));
        let child = format!("{rel}/{name}");
        println!("cargo:rerun-if-changed={}", path.display());
        if path.is_dir() {
            walk(&path, &child, files);
        } else {
            files.push((child, path));
        }
    }
}
