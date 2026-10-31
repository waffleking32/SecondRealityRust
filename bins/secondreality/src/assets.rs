//! The demo's media, embedded in the binary by `build.rs`: every file under the repository's
//! `assets/` (but `PROVENANCE.md`), so a run reads nothing from disk and works from any
//! working directory. Paths are repository-relative with forward slashes, e.g.
//! `assets/ALKU/FONA.UH`.

use sr_assets::Image;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// The bytes of the embedded `path`, if it is one.
pub(crate) fn get(path: &str) -> Option<&'static [u8]> {
    FILES
        .binary_search_by(|(name, _)| (*name).cmp(path))
        .ok()
        .map(|index| FILES[index].1)
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
            .all(|(name, _)| name.starts_with("assets/") && !name.contains('\\')));
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
        for (name, bytes) in FILES {
            let path = format!("../../{name}");
            let disk = std::fs::read(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
            assert!(disk == *bytes, "{name}");
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
