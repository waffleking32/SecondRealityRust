//! `xtask frames`: look at a directory of dumped frames.
//!
//! `compare` answers "how far apart are these two frame sets" with a number.
//! This answers the other question — "what do the port's frames *look like*" —
//! and it exists because a phase that cannot diff a part still has to see it:
//! TUNNELI's reference has no settled content at two of its four keyframes
//! (`docs/fidelity/reference-indices.md` §1.8), so its frames are read by a
//! person, not by a percentage.
//!
//! **The walk is `compare`'s**, [`crate::compare::capture_directories`],
//! directly: a directory holding a `manifest.json` is a capture and so is every
//! directory below it, which is what makes `--dir out/p2-tunneli` and `--dir
//! reference/parts` both work without a second traversal that could come to
//! disagree with the one every measurement uses. The layout is
//! [`crate::sheet`]'s, the same one `reference capture --sheet` and
//! `assets verify --sheet` draw, so a sheet reads the same wherever it came
//! from — and nothing is scaled, so a 320×400 tile is drawn at 320×400.
//!
//! Two rules the capture path set and this keeps: **the sheet is written from
//! the files on disk** (a rendering that showed anything else could show
//! something the dump did not write), and **nothing is written inside the
//! directory being read** — a sheet among the frames is a file every later
//! diff would have to step over.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::compare;
use crate::sheet::{self, Tile};

/// What `frames sheet` was given.
pub struct SheetOptions {
    /// The directory (or tree) of dumped frames to read.
    pub dir: PathBuf,
    /// Where the PNG goes.
    pub out: PathBuf,
}

/// `frames sheet --dir <DIR> --out <PNG>`: every frame under `dir` as one PNG,
/// in the walk's order, each capture's frames in the order its manifest lists
/// them.
///
/// Returns the exit code the process should end with: 0 when a sheet was
/// written. A sheet that cannot be built at all — no directory, no manifest
/// anywhere below it, no frames, a frame file that cannot be read, an `--out`
/// inside the tree being read — is an `Err`, which `main` gives exit 2.
pub fn sheet(options: &SheetOptions) -> Result<ExitCode, Box<dyn std::error::Error>> {
    // The frames being read are inputs: a sheet written among them would be a
    // file every later diff has to step over — the same rule `compare`'s
    // `--out` and the capture's `--sheet` already follow.
    compare::refuse_output_inside(
        &options.out,
        &options.dir,
        "--out",
        "that is the directory whose frames are being read, and a sheet belongs beside it, not \
         inside it",
        "Pass --out outside --dir, e.g. out/<part>-sheet.png",
    )?;

    let tiles = read_tiles(&options.dir)?;
    let columns = sheet::columns_for(tiles.len());
    let (width, height, rgb) = sheet::contact_sheet(&tiles, columns);
    sheet::write_png(&options.out, width, height, &rgb)
        .map_err(|error| format!("{}: {error}", options.out.display()))?;

    println!(
        "sheet {}: {} tile(s), {columns} column(s), {width}x{height}",
        options.out.display(),
        tiles.len()
    );
    for (index, tile) in tiles.iter().enumerate() {
        let (row, column) = sheet::position(index, columns);
        println!("  r{row}c{column} {}", tile.label);
    }
    Ok(ExitCode::SUCCESS)
}

/// Every frame under `dir`, as tiles, in the order the sheet lays them out.
///
/// The frames are read from the files their manifests name, and each tile's
/// geometry is the frame's own (`FrameRef::geometry`: a dump carries each
/// frame's size, because a part can change the view mid-run) falling back to
/// the manifest's.
fn read_tiles(dir: &Path) -> Result<Vec<Tile>, Box<dyn std::error::Error>> {
    let mut tiles = Vec::new();
    for capture in compare::capture_directories(dir).map_err(|error| error.to_string())? {
        let manifest =
            compare::read_manifest(&capture.dir).map_err(|error| error.to_string())?;
        for frame in &manifest.frames {
            let file = capture.dir.join(&frame.rgb);
            let rgb =
                fs::read(&file).map_err(|error| format!("{}: {error}", file.display()))?;
            let (width, height) = frame.geometry(&manifest);
            tiles.push(Tile {
                width: u32::from(width),
                height: u32::from(height),
                rgb,
                // The capture it came from, its index and its file name: a
                // sheet's tiles are named by what is printed beside them,
                // because nothing can draw text into the PNG.
                label: if capture.name.is_empty() {
                    format!("frame {} ({})", frame.index, frame.rgb)
                } else {
                    format!("{}: frame {} ({})", capture.name, frame.index, frame.rgb)
                },
            });
        }
    }
    if tiles.is_empty() {
        return Err(format!(
            "{} holds no frames: every capture below it has an empty manifest",
            dir.display()
        )
        .into());
    }
    Ok(tiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory of this test's own.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("xtask-frames-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A frame of `width` x `height`, every pixel `value`.
    fn frame(width: u32, height: u32, value: u8) -> Vec<u8> {
        vec![value; (width * height * 3) as usize]
    }

    /// A capture directory: `frames` as `(index, value)` pairs, each a
    /// `width` x `height` RGB frame, in the dump manifest's own shape.
    fn capture(dir: &Path, width: u32, height: u32, frames: &[(u32, u8)]) {
        fs::create_dir_all(dir).unwrap();
        let mut entries = Vec::new();
        for (index, value) in frames {
            let name = format!("frame_{index:06}.rgb");
            fs::write(dir.join(&name), frame(width, height, *value)).unwrap();
            entries.push(format!("{{\"index\": {index}, \"rgb\": \"{name}\"}}"));
        }
        let entries = entries.join(", ");
        fs::write(
            dir.join("manifest.json"),
            format!(
                "{{\"version\": 1, \"width\": {width}, \"height\": {height}, \
                 \"frames\": [{entries}]}}"
            ),
        )
        .unwrap();
    }

    fn sheet_of(dir: &Path, name: &str) -> PathBuf {
        dir.parent().unwrap().join(format!("{name}.png"))
    }

    /// The PNG as `(width, height, rgb)`.
    fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(fs::File::open(path).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        buffer.truncate(info.buffer_size());
        (info.width, info.height, buffer)
    }

    #[test]
    fn a_dump_becomes_one_sheet_of_its_frames() {
        // The whole point of the command: a directory of dumped frames, one
        // PNG. The sheet is exactly what `sheet::contact_sheet` builds from
        // the same frames in the same order — so the test is that the *reading*
        // (the walk, the manifests, the files) is right, and the layout is
        // `sheet`'s and already tested there.
        let dir = scratch("one");
        capture(&dir, 4, 2, &[(0, 0x11), (1, 0x22), (2, 0x33)]);
        let out = sheet_of(&dir, "one");

        assert_eq!(sheet(&SheetOptions { dir: dir.clone(), out: out.clone() }).unwrap(), ExitCode::SUCCESS);

        let expected_tiles: Vec<Tile> = [(0u32, 0x11u8), (1, 0x22), (2, 0x33)]
            .iter()
            .map(|(index, value)| Tile {
                width: 4,
                height: 2,
                rgb: frame(4, 2, *value),
                label: format!("frame {index} (frame_{index:06}.rgb)"),
            })
            .collect();
        let (width, height, rgb) = sheet::contact_sheet(&expected_tiles, 2);
        assert_eq!(read_png(&out), (width, height, rgb));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_tree_of_captures_sheets_them_all_in_the_walks_order() {
        // `--dir` may be a tree — `compare`'s own walk — and the tiles follow
        // it: captures sorted by name, and each capture's frames in the order
        // its manifest lists them.
        let dir = scratch("tree");
        capture(&dir.join("b"), 2, 1, &[(7, 0x77)]);
        capture(&dir.join("a"), 2, 1, &[(3, 0x33), (4, 0x44)]);
        let out = sheet_of(&dir, "tree");

        assert_eq!(sheet(&SheetOptions { dir: dir.clone(), out: out.clone() }).unwrap(), ExitCode::SUCCESS);

        let expected_tiles = vec![
            Tile { width: 2, height: 1, rgb: frame(2, 1, 0x33), label: "a: frame 3 (frame_000003.rgb)".into() },
            Tile { width: 2, height: 1, rgb: frame(2, 1, 0x44), label: "a: frame 4 (frame_000004.rgb)".into() },
            Tile { width: 2, height: 1, rgb: frame(2, 1, 0x77), label: "b: frame 7 (frame_000007.rgb)".into() },
        ];
        let (width, height, rgb) = sheet::contact_sheet(&expected_tiles, 2);
        assert_eq!(read_png(&out), (width, height, rgb));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_directory_with_no_manifest_is_refused() {
        // A sheet of nothing would be a picture nobody can read, and the
        // message has to name the directory that held no capture.
        let dir = scratch("empty");
        fs::write(dir.join("frame_000001.rgb"), frame(2, 2, 1)).unwrap();
        let error = sheet(&SheetOptions { dir: dir.clone(), out: sheet_of(&dir, "empty") })
            .unwrap_err()
            .to_string();
        assert!(error.contains("manifest.json"), "{error}");
        assert!(!sheet_of(&dir, "empty").exists(), "no PNG for a sheet that was refused");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_capture_whose_manifest_names_no_frames_is_refused() {
        // `compare`'s walk finds the capture and the manifest parses; there is
        // simply nothing to draw. A sheet of no tiles is not a picture, and
        // `write_png` refuses a 0x0 one — so the refusal has to come from here,
        // naming the directory, rather than from the writer.
        let dir = scratch("no-frames");
        capture(&dir, 2, 2, &[]);
        let error = sheet(&SheetOptions { dir: dir.clone(), out: sheet_of(&dir, "no-frames") })
            .unwrap_err()
            .to_string();
        assert!(error.contains("holds no frames"), "{error}");
        assert!(!sheet_of(&dir, "no-frames").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_frame_file_the_manifest_names_but_that_is_missing_is_refused() {
        // `verify` catches this for the reference tree; a dump can be read
        // straight after a run, and a hole in it must not be a blank tile.
        let dir = scratch("hole");
        capture(&dir, 2, 2, &[(1, 0x11), (2, 0x22)]);
        fs::remove_file(dir.join("frame_000002.rgb")).unwrap();
        let error = sheet(&SheetOptions { dir: dir.clone(), out: sheet_of(&dir, "hole") })
            .unwrap_err()
            .to_string();
        assert!(error.contains("frame_000002.rgb"), "{error}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_sheet_inside_the_directory_it_reads_is_refused() {
        // The frames are inputs: writing among them leaves a file every later
        // diff steps over — the rule `compare --out` and `capture --sheet`
        // already carry.
        let dir = scratch("inside");
        capture(&dir, 2, 2, &[(1, 0x11)]);
        let error = sheet(&SheetOptions { dir: dir.clone(), out: dir.join("sheet.png") })
            .unwrap_err()
            .to_string();
        assert!(error.contains("inside"), "{error}");
        assert!(!dir.join("sheet.png").exists());
        let _ = fs::remove_dir_all(&dir);
    }
}
