//! `frames stills`: the wiki's pictures, rendered by the port and written as PNGs.
//!
//! [`STILLS`] names every picture under `wiki/images/`: the run that shows it and the frame.
//! Each distinct run goes once through the release binary (`--headless --dump-frames`), and
//! each frame is written as `<page>-<index>.png`, magnified by pixel duplication to the
//! 640×400 the window shows (a 320×200 frame doubles both ways, a 320×400 one doubles only its
//! columns), so the pictures can be regenerated from the tree at any time.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

use crate::capture::{read_frame, read_manifest};
use crate::rediff::build_release;
use crate::sheet::write_png;

/// One wiki picture: the page it belongs to, the run's arguments before `--headless …`, and
/// the frame index to take from that run.
pub struct Still {
    pub page: &'static str,
    pub args: &'static [&'static str],
    pub index: u32,
}

/// The whole demo from the start, as `SECOND.EXE` plays it (boot-numbered frames).
const DEMO: &[&str] = &["--no-setup"];

/// The starfield that follows U2E in the demo, on its own (part-numbered frames).
const DDSTARS: &[&str] = &["--part", "ddstars"];

/// The vault's grid effect, on its own.
const GRID: &[&str] = &["--part", "grid"];

/// The vault's hard effect, on its own.
const HARD: &[&str] = &["--part", "hard"];

/// The vault's twist effect, on its own.
const TWIST: &[&str] = &["--part", "twist"];

/// Every picture the wiki shows, in the demo's order.
#[rustfmt::skip]
pub const STILLS: &[Still] = &[
    Still { page: "alku", args: DEMO, index: 1700 },
    Still { page: "alku", args: DEMO, index: 2700 },
    Still { page: "alku", args: DEMO, index: 4000 },
    Still { page: "alku", args: DEMO, index: 5100 },
    Still { page: "u2a", args: DEMO, index: 6500 },
    Still { page: "u2a", args: DEMO, index: 7100 },
    Still { page: "u2a", args: DEMO, index: 7300 },
    Still { page: "pam", args: DEMO, index: 7700 },
    Still { page: "pam", args: DEMO, index: 7800 },
    Still { page: "beglogo", args: DEMO, index: 8000 },
    Still { page: "glenz", args: DEMO, index: 8100 },
    Still { page: "glenz", args: DEMO, index: 8600 },
    Still { page: "glenz", args: DEMO, index: 9400 },
    Still { page: "glenz", args: DEMO, index: 9800 },
    Still { page: "glenz", args: DEMO, index: 10000 },
    Still { page: "tunneli", args: DEMO, index: 11300 },
    Still { page: "tunneli", args: DEMO, index: 11700 },
    Still { page: "tunneli", args: DEMO, index: 12000 },
    Still { page: "techno", args: DEMO, index: 12300 },
    Still { page: "techno", args: DEMO, index: 12700 },
    Still { page: "techno", args: DEMO, index: 14400 },
    Still { page: "techno", args: DEMO, index: 15800 },
    Still { page: "panicend", args: DEMO, index: 16200 },
    Still { page: "mntscrl", args: DEMO, index: 16700 },
    Still { page: "mntscrl", args: DEMO, index: 17200 },
    Still { page: "mntscrl", args: DEMO, index: 17800 },
    Still { page: "mntscrl", args: DEMO, index: 18400 },
    Still { page: "lnszoom", args: DEMO, index: 19400 },
    Still { page: "lnszoom", args: DEMO, index: 19500 },
    Still { page: "lnszoom", args: DEMO, index: 20600 },
    Still { page: "lnszoom", args: DEMO, index: 21300 },
    Still { page: "plzpart", args: DEMO, index: 22300 },
    Still { page: "plzpart", args: DEMO, index: 23000 },
    Still { page: "plzpart", args: DEMO, index: 24500 },
    Still { page: "plzpart", args: DEMO, index: 24800 },
    Still { page: "minvball", args: DEMO, index: 27000 },
    Still { page: "minvball", args: DEMO, index: 27600 },
    Still { page: "minvball", args: DEMO, index: 28100 },
    Still { page: "minvball", args: DEMO, index: 28800 },
    Still { page: "rayscrl", args: DEMO, index: 29200 },
    Still { page: "rayscrl", args: DEMO, index: 29500 },
    Still { page: "rayscrl", args: DEMO, index: 30200 },
    Still { page: "rayscrl", args: DEMO, index: 30700 },
    Still { page: "3dsinfld", args: DEMO, index: 31300 },
    Still { page: "3dsinfld", args: DEMO, index: 31800 },
    Still { page: "3dsinfld", args: DEMO, index: 31900 },
    Still { page: "3dsinfld", args: DEMO, index: 32900 },
    Still { page: "3dsinfld", args: DEMO, index: 33100 },
    Still { page: "jplogo", args: DEMO, index: 33300 },
    Still { page: "jplogo", args: DEMO, index: 33400 },
    Still { page: "jplogo", args: DEMO, index: 33500 },
    Still { page: "jplogo", args: DEMO, index: 34000 },
    Still { page: "u2e", args: DEMO, index: 34100 },
    Still { page: "u2e", args: DEMO, index: 34200 },
    Still { page: "u2e", args: DEMO, index: 35100 },
    Still { page: "u2e", args: DEMO, index: 35200 },
    Still { page: "u2e", args: DEMO, index: 37900 },
    Still { page: "ddstars", args: DDSTARS, index: 600 },
    Still { page: "ddstars", args: DDSTARS, index: 1500 },
    Still { page: "ddstars", args: DDSTARS, index: 1600 },
    Still { page: "ddstars", args: DDSTARS, index: 2900 },
    Still { page: "endlogo", args: DEMO, index: 38000 },
    Still { page: "endlogo", args: DEMO, index: 38100 },
    Still { page: "endlogo", args: DEMO, index: 38300 },
    Still { page: "cred", args: DEMO, index: 39300 },
    Still { page: "cred", args: DEMO, index: 39800 },
    Still { page: "cred", args: DEMO, index: 40000 },
    Still { page: "cred", args: DEMO, index: 41600 },
    Still { page: "endscrl", args: DEMO, index: 46600 },
    Still { page: "endscrl", args: DEMO, index: 47500 },
    Still { page: "endscrl", args: DEMO, index: 49700 },
    Still { page: "grid", args: GRID, index: 300 },
    Still { page: "grid", args: GRID, index: 700 },
    Still { page: "grid", args: GRID, index: 1300 },
    Still { page: "hard", args: HARD, index: 300 },
    Still { page: "hard", args: HARD, index: 800 },
    Still { page: "hard", args: HARD, index: 1500 },
    Still { page: "twist", args: TWIST, index: 300 },
    Still { page: "twist", args: TWIST, index: 1200 },
    Still { page: "twist", args: TWIST, index: 2900 },
];

/// The window's picture size; every still is magnified to it.
const SHOWN: (u32, u32) = (640, 400);

/// `<page>-<index>.png`, the still's file name under `wiki/images/`.
pub fn file_name(still: &Still) -> String {
    format!("{}-{}.png", still.page, still.index)
}

/// `frames stills --out <DIR>`: render every still into `out`. `Err` (exit 2): the build, a
/// run or a write failed.
pub fn command(binary: Option<PathBuf>, out: &Path) -> Result<ExitCode, String> {
    let binary = match binary {
        Some(binary) => binary,
        None => build_release()?,
    };
    fs::create_dir_all(out).map_err(|error| format!("{}: {error}", out.display()))?;
    let mut runs: BTreeMap<&[&str], Vec<&Still>> = BTreeMap::new();
    for still in STILLS {
        runs.entry(still.args).or_default().push(still);
    }
    let scratch = Path::new("out/stills");
    for (number, (args, stills)) in runs.iter().enumerate() {
        let dump = scratch.join(number.to_string());
        let _ = fs::remove_dir_all(&dump);
        let mut indices: Vec<u32> = stills.iter().map(|still| still.index).collect();
        indices.sort_unstable();
        indices.dedup();
        let list = indices
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let status = Command::new(&binary)
            .args(*args)
            .args(["--headless", "--dump-frames", &list, "--dump-dir"])
            .arg(&dump)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .status()
            .map_err(|error| format!("cannot run {}: {error}", binary.display()))?;
        if !status.success() {
            return Err(format!(
                "{} {} exited with {status}",
                binary.display(),
                args.join(" ")
            ));
        }
        let manifest = read_manifest(&dump)?;
        for still in stills {
            let frame = manifest
                .frames
                .iter()
                .find(|frame| frame.index == still.index)
                .ok_or_else(|| format!("the run dumped no frame {}", still.index))?;
            let (width, height) = frame.geometry(&manifest);
            let rgb = read_frame(&dump, &frame.rgb)?;
            let (shown, picture) = magnify(u32::from(width), u32::from(height), &rgb);
            let path = out.join(file_name(still));
            write_png(&path, shown.0, shown.1, &picture)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            println!(
                "{} ({width}x{height} frame {})",
                path.display(),
                still.index
            );
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// `rgb` (`width`×`height`) duplicated up to [`SHOWN`], or left as it is when it does not
/// divide it.
fn magnify(width: u32, height: u32, rgb: &[u8]) -> ((u32, u32), Vec<u8>) {
    if width == 0 || height == 0 || SHOWN.0 % width != 0 || SHOWN.1 % height != 0 {
        return ((width, height), rgb.to_vec());
    }
    let (sx, sy) = ((SHOWN.0 / width) as usize, (SHOWN.1 / height) as usize);
    let mut picture = Vec::with_capacity(SHOWN.0 as usize * SHOWN.1 as usize * 3);
    for row in rgb.chunks_exact(width as usize * 3) {
        let wide: Vec<u8> = row
            .chunks_exact(3)
            .flat_map(|pixel| pixel.repeat(sx))
            .collect();
        for _ in 0..sy {
            picture.extend_from_slice(&wide);
        }
    }
    (SHOWN, picture)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every still is one picture under `wiki/images/`, and every picture there is a still,
    /// so the directory is exactly what `frames stills` regenerates.
    #[test]
    fn the_stills_are_exactly_the_wikis_pictures() {
        let mut names: Vec<String> = STILLS.iter().map(file_name).collect();
        let count = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), count, "two stills share a file name");
        let images = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wiki/images");
        let mut on_disk: Vec<String> = fs::read_dir(&images)
            .unwrap_or_else(|error| panic!("{}: {error}", images.display()))
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".png"))
            .collect();
        on_disk.sort();
        assert_eq!(on_disk, names);
    }

    #[test]
    fn a_frame_is_duplicated_up_to_the_windows_size() {
        let rgb: Vec<u8> = (0..320 * 200).flat_map(|i| [i as u8, 0, 0]).collect();
        let ((width, height), picture) = magnify(320, 200, &rgb);
        assert_eq!((width, height), (640, 400));
        assert_eq!(&picture[..6], &[0, 0, 0, 0, 0, 0]);
        assert_eq!(&picture[6..9], &[1, 0, 0]);
        assert_eq!(
            &picture[640 * 3..640 * 3 + 3],
            &[0, 0, 0],
            "the second row repeats the first"
        );
        let ((width, height), _) = magnify(320, 400, &vec![0; 320 * 400 * 3]);
        assert_eq!((width, height), (640, 400));
        let ((width, height), _) = magnify(640, 400, &vec![0; 640 * 400 * 3]);
        assert_eq!((width, height), (640, 400));
    }
}
