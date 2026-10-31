//! The demo's media, embedded in the binary by `build.rs`: every file under the repository's
//! `assets/` (but `PROVENANCE.md`), so a run reads nothing from disk and works from any
//! working directory. Paths are repository-relative with forward slashes, e.g.
//! `assets/ALKU/FONA.UH`.
//!
//! The tree is stored as one raw LZMA stream, which [`unpack`] decompresses whole, once: `main`
//! calls it before a run starts, so no frame waits on a decode (docs/design.md §9 has the time).
//! A caller borrows `&'static` bytes of the unpacked tree.

use std::io::Read as _;
use std::sync::OnceLock;

use lzma_rust2::LzmaReader;
use sr_assets::Image;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The unpacked tree, which `FILES` indexes.
static TREE: OnceLock<Box<[u8]>> = OnceLock::new();

/// The unpacked tree, decompressed on the first call. The stream is build.rs's own output, so a
/// failure is a build bug, not a bad input.
pub(crate) fn unpack() -> &'static [u8] {
    TREE.get_or_init(|| {
        // The reader rounds the dictionary up to LZMA's minimum, as build.rs's encoder did.
        let dict_size = u32::try_from(UNPACKED_LEN).expect("a tree under 4 GiB");
        let mut tree = vec![0; UNPACKED_LEN];
        LzmaReader::new(PACKED, UNPACKED_LEN as u64, LC, LP, PB, dict_size, None)
            .and_then(|mut reader| reader.read_exact(&mut tree))
            .unwrap_or_else(|error| panic!("the embedded assets do not decode: {error}"));
        tree.into_boxed_slice()
    })
}

/// The bytes of the embedded `path`, if it is one.
pub(crate) fn get(path: &str) -> Option<&'static [u8]> {
    let index = FILES.binary_search_by(|(name, ..)| (*name).cmp(path)).ok()?;
    let (_, start, len) = FILES[index];
    Some(&unpack()[start..start + len])
}

/// [`get`], or an error naming `path`.
pub(crate) fn read(path: &str) -> Result<&'static [u8], String> {
    get(path).ok_or_else(|| format!("{path}: not embedded in this binary"))
}

/// The picture `path` holds ([`sr_assets::load_any`]), or an error naming `path`.
pub(crate) fn image(path: &str) -> Result<Image, String> {
    sr_assets::load_any(read(path)?).map_err(|error| format!("{path}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_sorted_and_holds_the_tree() {
        assert!(
            FILES.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "sorted, no duplicates"
        );
        assert!(FILES
            .iter()
            .all(|(name, ..)| name.starts_with("assets/") && !name.contains('\\')));
        assert_eq!(
            get("assets/PROVENANCE.md"),
            None,
            "the documentation is not embedded"
        );
        assert_eq!(get("assets/ALKU"), None, "directories are not entries");
        assert!(read("assets/NO/SUCH.FILE")
            .unwrap_err()
            .contains("assets/NO/SUCH.FILE"));
    }

    #[test]
    fn every_embedded_file_is_the_trees_copy() {
        for (name, ..) in FILES {
            let path = format!("../../{name}");
            let disk = std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
            assert!(disk == get(name).unwrap(), "{name}");
        }
        let mut count = 0;
        count_files(std::path::Path::new("../../assets"), &mut count);
        assert_eq!(
            FILES.len() + 1,
            count,
            "every file under assets/ but PROVENANCE.md"
        );
    }

    fn count_files(dir: &std::path::Path, count: &mut usize) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                count_files(&path, count);
            } else {
                *count += 1;
            }
        }
    }
}
