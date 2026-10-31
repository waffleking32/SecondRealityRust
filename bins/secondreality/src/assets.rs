//! The demo's media, embedded in the binary by `build.rs`: every file under the repository's
//! `assets/` (but `PROVENANCE.md`), so a run reads nothing from disk and works from any
//! working directory. Paths are repository-relative with forward slashes, e.g.
//! `assets/ALKU/FONA.UH`.
//!
//! Each file is stored as a raw LZMA stream and decompressed the first time it is asked for, then
//! kept for the rest of the run, so a caller still borrows `&'static` bytes. The largest,
//! MUSIC1.S3M, takes about 14 ms in a release build, the whole tree about 0.1 s.

use std::io::Read as _;
use std::sync::OnceLock;

use lzma_rust2::LzmaReader;
use sr_assets::Image;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// `FILES[i]`'s bytes once decompressed.
static DECODED: [OnceLock<Box<[u8]>>; FILES.len()] = [const { OnceLock::new() }; FILES.len()];

/// The bytes of the embedded `path`, if it is one.
pub(crate) fn get(path: &str) -> Option<&'static [u8]> {
    FILES
        .binary_search_by(|(name, ..)| (*name).cmp(path))
        .ok()
        .map(|index| &**DECODED[index].get_or_init(|| decode(index)))
}

/// `FILES[index]`'s stream, decompressed. The stream is build.rs's own output, so a failure is a
/// build bug, not a bad input.
fn decode(index: usize) -> Box<[u8]> {
    let (name, start, end, len) = FILES[index];
    let dict_size = u32::try_from(len.max(DICT_MIN)).expect("an asset under 4 GiB");
    let mut bytes = vec![0; len];
    LzmaReader::new(&PACKED[start..end], len as u64, LC, LP, PB, dict_size, None)
        .and_then(|mut reader| reader.read_exact(&mut bytes))
        .unwrap_or_else(|error| panic!("{name}: the embedded stream does not decode: {error}"));
    bytes.into_boxed_slice()
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
        assert!(
            FILES.windows(2).all(|pair| pair[0].2 == pair[1].1)
                && FILES[0].1 == 0
                && FILES[FILES.len() - 1].2 == PACKED.len(),
            "the streams tile PACKED"
        );
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
