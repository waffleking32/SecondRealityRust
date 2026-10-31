//! What every `xtask` integration test shares: the built binary, the repository, the fixtures
//! and a scratch directory. Tests write only under the temporary directory: `reference/` and
//! `assets/` are input-only, and so are the fixtures.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// One run of the built `xtask`.
pub struct Run {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Run {
    /// Both streams: reports go to stdout, refusals to stderr.
    pub fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// The repository root, `xtask/..`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// `xtask/tests/fixtures/<name>`.
pub fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
}

/// A scratch directory of this test crate's own, emptied first.
pub fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "xtask-{}-{}-{name}",
        env!("CARGO_CRATE_NAME"),
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// The built `xtask`, run from `dir`.
pub fn xtask_in(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", env!("CARGO_BIN_EXE_xtask")));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The built `xtask`, run from the repository root, where the commands' relative defaults
/// point.
pub fn xtask(args: &[&str]) -> Run {
    xtask_in(&repo_root(), args)
}

/// Copy a tree, for a test that changes one of its own.
pub fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}
