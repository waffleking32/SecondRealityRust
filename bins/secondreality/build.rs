//! Embeds the repository's `assets/` in the binary, so a run needs no files beside it.
//!
//! Compresses every file under `assets/` (except `PROVENANCE.md`, the tree's documentation) to a
//! raw LZMA stream, writes the streams end to end to `$OUT_DIR/assets.lzma`, and writes
//! `$OUT_DIR/assets.rs`: `PACKED`, that file's `include_bytes!`, and `FILES`, each asset's
//! (repo-relative path, range in `PACKED`, decompressed length), sorted by path so
//! `src/assets.rs` can binary-search it. Paths use forward slashes on every host. Every directory
//! and file is a `rerun-if-changed`, so adding, removing or changing an asset rebuilds the binary.
//!
//! The streams carry no `.lzma` header (13 bytes each): the properties are the constants below
//! and the length is in `FILES`. docs/design.md §9 has why LZMA.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use lzma_rust2::{EncodeMode, LzmaOptions, LzmaWriter, MfType};

/// The tree's documentation, parsed by `xtask assets`; not media.
const SKIPPED: &str = "assets/PROVENANCE.md";

/// LZMA's literal context bits, literal position bits and position bits. lc 4, pb 0 beat the
/// default lc 3, pb 2 on this tree by 15 KB: it is byte-oriented data, little of it aligned.
const LC: u32 = 4;
const LP: u32 = 0;
const PB: u32 = 0;

fn main() {
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let assets = manifest
        .join("../../assets")
        .canonicalize()
        .expect("the repository's assets/");
    println!("cargo:rerun-if-changed={}", assets.display());

    let mut files = Vec::new();
    walk(&assets, "assets", &mut files);
    files.retain(|(rel, _)| rel != SKIPPED);
    files.sort();

    let mut packed = Vec::new();
    let mut out = format!(
        "const LC: u32 = {LC};\nconst LP: u32 = {LP};\nconst PB: u32 = {PB};\n\
         /// Every embedded asset's raw LZMA stream, end to end.\n\
         static PACKED: &[u8] = include_bytes!(concat!(env!(\"OUT_DIR\"), \"/assets.lzma\"));\n\
         /// Every embedded asset as (repository-relative path, its stream's start and end in\n\
         /// `PACKED`, its decompressed length), sorted by path.\n\
         static FILES: [(&str, usize, usize, usize); {}] = [\n",
        files.len()
    );
    for (rel, path) in &files {
        let bytes =
            std::fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let start = packed.len();
        compress(&mut packed, &bytes);
        // `{:?}` writes a Rust string literal, backslashes and quotes escaped.
        writeln!(out, "    ({rel:?}, {start}, {}, {}),", packed.len(), bytes.len()).unwrap();
    }
    out.push_str("];\n");

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let blob = out_dir.join("assets.lzma");
    std::fs::write(&blob, packed).unwrap_or_else(|error| panic!("{}: {error}", blob.display()));
    let dest = out_dir.join("assets.rs");
    std::fs::write(&dest, out).unwrap_or_else(|error| panic!("{}: {error}", dest.display()));
}

/// Appends `bytes`' raw LZMA stream to `packed`: no header and no end marker, a dictionary the
/// size of the file (at least `DICT_SIZE_MIN`), and xz -9e's search (BT4, nice length 273).
/// `src/assets.rs` decodes it with the same properties.
fn compress(packed: &mut Vec<u8>, bytes: &[u8]) {
    let dict_size = u32::try_from(bytes.len().max(lzma_rust2::DICT_SIZE_MIN as usize))
        .expect("an asset under 4 GiB");
    let options = LzmaOptions::new(dict_size, LC, LP, PB, EncodeMode::Normal, 273, MfType::Bt4, 0);
    let mut writer = LzmaWriter::new(packed, &options, false, false, Some(bytes.len() as u64))
        .expect("the LZMA encoder");
    std::io::Write::write_all(&mut writer, bytes).expect("compressing into memory");
    writer.finish().expect("compressing into memory");
}

/// Every file below `dir`, whose repository-relative name is `rel`, as (`rel/...`, path).
fn walk(dir: &Path, rel: &str, files: &mut Vec<(String, PathBuf)>) {
    let entries =
        std::fs::read_dir(dir).unwrap_or_else(|error| panic!("{}: {error}", dir.display()));
    for entry in entries {
        let entry = entry.expect("a directory entry");
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
