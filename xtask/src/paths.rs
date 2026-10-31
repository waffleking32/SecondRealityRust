//! The guard every writer in this tool goes through: an output may not land inside an input
//! (`reference/`, `assets/`, a tree being read), resolved paths and all.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// `path` with symlinks, `.` and `..` resolved as far as it exists: the deepest existing
/// ancestor is canonicalised, the rest appended and collapsed lexically. The collapse is what
/// stops a `..` behind a component that does not exist yet (`missing/../ref/nested`) from
/// escaping a string comparison while the write still lands inside `ref` (D16). The components
/// after the deepest existing ancestor cannot be symlinks, so collapsing them lexically is what
/// the filesystem does with them.
pub(crate) fn resolve(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot read the working directory: {e}"))?
            .join(path)
    };
    for ancestor in absolute.ancestors() {
        if let Ok(resolved) = fs::canonicalize(ancestor) {
            let rest = absolute.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return Ok(collapse(&resolved.join(rest)));
        }
    }
    Ok(collapse(&absolute))
}

/// `path` without `.` components and with each `..` taken off the component before it,
/// without reading the filesystem.
fn collapse(path: &Path) -> PathBuf {
    let absolute = path.is_absolute();
    let mut collapsed = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // `/..` is `/`; a leading `..` on a relative path stays.
                if !collapsed.pop() && !absolute {
                    collapsed.push(Component::ParentDir.as_os_str());
                }
            }
            component => collapsed.push(component.as_os_str()),
        }
    }
    collapsed
}

/// Refuse to write `out` inside `input`. `flag` is the option to change, `why` what would be
/// damaged, and `elsewhere` where to put it instead, so the message is usable on its own.
pub(crate) fn refuse_output_inside(
    out: &Path,
    input: &Path,
    flag: &str,
    why: &str,
    elsewhere: &str,
) -> Result<(), String> {
    let out = resolve(out)?;
    let input = resolve(input)?;
    if out.starts_with(&input) {
        return Err(format!(
            "{flag} {} is inside the input tree {}: {why}. {elsewhere}",
            out.display(),
            input.display()
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dotdot_behind_a_missing_component_resolves_inside_the_input() {
        let parent = std::env::temp_dir().join(format!("xtask-paths-{}", std::process::id()));
        let input = parent.join("ref");
        fs::create_dir_all(&input).unwrap();

        let out = parent.join("missing/../ref/nested");
        assert_eq!(resolve(&out).unwrap(), resolve(&input).unwrap().join("nested"));
        let error = refuse_output_inside(&out, &input, "--out", "why", "elsewhere").unwrap_err();
        assert!(error.contains("ref/nested"), "{error}");

        // The input itself, and a trailing `/.`, are inside it too; a sibling is not.
        assert!(refuse_output_inside(&input, &input, "--out", "", "").is_err());
        assert!(refuse_output_inside(&input.join("."), &input, "--out", "", "").is_err());
        assert!(refuse_output_inside(&parent.join("other"), &input, "--out", "", "").is_ok());
        assert!(!parent.join("missing").exists());
        let _ = fs::remove_dir_all(&parent);
    }
}
