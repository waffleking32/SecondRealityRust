//! `xtask assets verify`: the curated tree against `assets/PROVENANCE.md`, every asset decoded,
//! and one contact sheet per part to look at.
//!
//! - `--tree` re-hashes every file against the record (and its `bytes` column), and reports a
//!   recorded file that is missing and a file nothing records.
//! - `--decode` opens every asset. A file whose own magic says it is a picture
//!   ([`sr_assets::sniff`]) must decode; the one recorded refusal is [`REFUSED_ON_RECORD`], and
//!   it decoding is a finding too. `.PAL`, `.ANI` and the Visu scene set (`VISU/C/SCENE/*`,
//!   read through `sr_assets::visu` as `VISU/C/MAIN.C` and `SAVE.C` name it, the `.0AB` with
//!   its `.00M`'s object count, `U2A.C:168-172`) are opened by their names. Other headerless
//!   files (`.U`, `.1BM`, `.EGA`) are reported, never decoded: only their part knows their
//!   geometry.
//! - `--sheet <DIR>` runs the decode pass and writes one PNG per part from its tiles; `<DIR>`
//!   may not be inside the tree.
//!
//! `PROVENANCE.md` is read by header name, never by position: a table counts only when its
//! header holds both `path` and `sha256` (the *Collapsed duplicates* and *Identical files*
//! tables do not), and `parts` is read where a table has it (the main table and A5's
//! *Recovered assets*). Exit codes: **0** checked out, **1** a finding, **2** could not run (no
//! tree, a malformed record, a refused `--sheet`, nothing asked for).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sr_assets::visu::{read_animation, read_master, read_object, read_scene_list};
use sr_assets::{load, load_frames, pal, sniff, Format, Image};
use sr_engine::{dac_expand, Palette};

use crate::sheet::{self, Tile};

/// The tree this checks, relative to where it is run from — the repository
/// root, as every other `xtask` default is.
pub const DEFAULT_TREE: &str = "assets";

/// The file the tree is recorded in, inside that tree.
pub const PROVENANCE_FILE: &str = "PROVENANCE.md";

/// The two columns a provenance table must have to be read as one. Everything
/// else — `origin`, `commit`, `bytes`, `licence`, A5's `blob`/`record`/`range` —
/// is the record's business, not this tool's.
const PATH_COLUMN: &str = "path";
const SHA256_COLUMN: &str = "sha256";

/// The column naming the file's length, present in the main table and absent
/// from A5's recovered one. Checked where it exists: a digest match means the
/// size matches too, so a size that does not is the record contradicting itself.
const BYTES_COLUMN: &str = "bytes";

/// The column naming the port's parts, present only in the main table.
const PARTS_COLUMN: &str = "parts";

/// Assets whose *refusal to decode* is the recorded outcome, each with the
/// record that says so.
///
/// `COMAN/COMBG.UH` is a truncated copy as it shipped: 32,768 bytes where its header's geometry
/// needs 64,016 (`docs/fidelity/asset-inventory.md` §E.2). `COMAN/MAIN.C:163,170,174` reads no
/// further than byte 29,582, so the part never needs the rest, and a refusal is the check
/// passing.
const REFUSED_ON_RECORD: &[(&str, &str)] = &[(
    "COMAN/COMBG.UH",
    "docs/fidelity/asset-inventory.md §E.2 and assets/PROVENANCE.md's note on the file: the copy \
     is truncated as it shipped, and a loader must refuse it rather than clip it",
)];

/// The width a file with no decoder is shown at: the demo's own screen width, so
/// a bare 64,000-byte screen lands at 320×200 in the sheet and reads as the
/// picture it is.
const BYTE_VIEW_WIDTH: usize = 320;

/// The most rows a byte view shows. 200 is a screen; a 600 KB music module at
/// 320 wide would be 1,800 rows of noise, and the label says what was cut.
const BYTE_VIEW_ROWS: usize = 200;

/// A palette is drawn as a strip, one column per entry — a palette should look
/// like a palette.
const PALETTE_STRIP_HEIGHT: u32 = 16;

/// Why the tool could not run at all.
#[derive(Debug)]
pub enum VerifyError {
    /// The tree, the provenance, or an output is not readable or not writable.
    Io(String),
    /// `PROVENANCE.md` is readable but is not the record this tool reads: no
    /// table with both a `path` and a `sha256` column, a row that does not fit
    /// its table, a path that is empty, a digest that is not a digest, or one
    /// path recorded twice.
    Malformed(String),
    /// Nothing was asked for — `--tree`, `--decode` and `--sheet` were all left
    /// out, so there is no check whose result an exit code could mean.
    NothingAsked,
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::Io(message) | VerifyError::Malformed(message) => f.write_str(message),
            VerifyError::NothingAsked => f.write_str(
                "nothing to check: pass --tree, --decode or --sheet <DIR> (running all three is \
                 the full check)",
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// What to check, straight off the command line.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Re-hash every file and compare it with `PROVENANCE.md`.
    pub tree: bool,
    /// Decode every asset that has a decoder, and report what each one is.
    pub decode: bool,
    /// Write one PNG per part, each of that part's assets a tile.
    pub sheet: Option<PathBuf>,
}

/// One row of a provenance table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The path under the tree, e.g. `ALKU/FONA.UH`.
    pub path: String,
    /// The recorded digest, 64 hex digits.
    pub sha256: String,
    /// The recorded length in bytes, where the table has a `bytes` column.
    pub bytes: Option<usize>,
    /// The port's parts that need the file, from the `parts` column — the main
    /// table and A5's recovered table both carry it. Empty for a row in a table
    /// without one, and a part is never invented for such a row: a file nobody
    /// records a part for is on nobody's sheet, and the tool says so rather than
    /// guessing from the directory it lives under.
    pub parts: Vec<String>,
    /// Which table the row was read from: the `##` heading it sat under.
    pub table: String,
}

/// A table's column positions, taken from its header row.
struct Columns {
    path: usize,
    sha256: usize,
    bytes: Option<usize>,
    parts: Option<usize>,
    /// The `##` heading the table sits under, so a row can say which table held
    /// it.
    table: String,
}

/// Read the provenance: every table whose header holds both a `path` and a
/// `sha256` column, in file order, read through those column *names*.
pub fn parse_provenance(text: &str) -> Result<Vec<Row>, VerifyError> {
    let mut rows: Vec<Row> = Vec::new();
    let mut columns: Option<Columns> = None;
    let mut heading = String::from("(no heading)");
    let mut header_seen = false;

    for (number, line) in text.lines().enumerate() {
        let number = number + 1;
        let line = line.trim_end();
        if let Some(rest) = line.strip_prefix('#') {
            heading = rest.trim_start_matches('#').trim().to_owned();
            columns = None;
            continue;
        }
        let Some(row) = line.strip_prefix('|') else {
            // Any non-table line ends the table before it: a later table brings
            // its own header.
            columns = None;
            continue;
        };
        let cells: Vec<String> = row
            .trim_end_matches('|')
            .split('|')
            .map(|cell| cell.trim().to_owned())
            .collect();
        // The `|---|` separator carries no names and no data.
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|c| matches!(c, '-' | ':' | ' ')))
        {
            continue;
        }
        let named = |name: &str| cells.iter().position(|cell| cell == name);
        match (&columns, named(PATH_COLUMN), named(SHA256_COLUMN)) {
            // A header: these two names are what make a table a provenance
            // table at all.
            (_, Some(path), Some(sha256)) => {
                columns = Some(Columns {
                    path,
                    sha256,
                    bytes: named(BYTES_COLUMN),
                    parts: named(PARTS_COLUMN),
                    table: heading.clone(),
                });
                header_seen = true;
            }
            // A data row of the table the last header opened.
            (Some(open), _, _) => {
                let cell = |index: usize| -> Result<String, VerifyError> {
                    cells.get(index).map(|cell| unquote(cell)).ok_or_else(|| {
                        VerifyError::Malformed(format!(
                            "{} line {number}: a row with {} cells where its table's header has \
                             at least {}",
                            PROVENANCE_FILE,
                            cells.len(),
                            index + 1
                        ))
                    })
                };
                let path = cell(open.path)?;
                if path.is_empty() {
                    return Err(VerifyError::Malformed(format!(
                        "{} line {number}: a row with an empty `{PATH_COLUMN}`",
                        PROVENANCE_FILE
                    )));
                }
                let sha256 = cell(open.sha256)?.to_lowercase();
                if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(VerifyError::Malformed(format!(
                        "{} line {number}: {path} has `{sha256}` in its `{SHA256_COLUMN}` column, \
                         which is not a SHA-256 digest",
                        PROVENANCE_FILE
                    )));
                }
                let parts = match open.parts {
                    Some(index) => parts_of(&cell(index)?),
                    None => Vec::new(),
                };
                let bytes = match open.bytes {
                    Some(index) => {
                        let value = cell(index)?;
                        Some(value.parse::<usize>().map_err(|_| {
                            VerifyError::Malformed(format!(
                                "{} line {number}: {path} has `{value}` in its `{BYTES_COLUMN}` \
                                 column, which is not a length in bytes",
                                PROVENANCE_FILE
                            ))
                        })?)
                    }
                    None => None,
                };
                rows.push(Row {
                    path,
                    sha256,
                    bytes,
                    parts,
                    table: open.table.clone(),
                });
            }
            // A table row that is neither a header nor inside a provenance
            // table: one of the record's other tables, which is read by people.
            (None, _, _) => {}
        }
    }

    if !header_seen {
        return Err(VerifyError::Malformed(format!(
            "{PROVENANCE_FILE} holds no table with both a `{PATH_COLUMN}` and a `{SHA256_COLUMN}` \
             column; this tool reads the record by those column names, never by position"
        )));
    }
    // One path, one row — anywhere in the file, not only next to it: the two
    // tables are read in file order and a path that appears in both would be
    // hashed twice and decoded twice, which is how a record that disagrees with
    // itself gets read as agreement.
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    for row in &rows {
        if let Some(first) = seen.insert(row.path.as_str(), row.table.as_str()) {
            return Err(VerifyError::Malformed(format!(
                "{PROVENANCE_FILE} records {} twice, under the \"{first}\" and \"{}\" tables",
                row.path, row.table
            )));
        }
    }
    Ok(rows)
}

/// A cell's value: the record wraps paths and digests in backticks.
fn unquote(cell: &str) -> String {
    cell.trim()
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
        .unwrap_or_else(|| cell.trim())
        .to_owned()
}

/// The `parts` cell: the port's part names, whitespace or comma separated. The
/// column is plural, and a part no file names has no sheet to write.
fn parts_of(cell: &str) -> Vec<String> {
    cell.split([',', ' ', '\t'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

/// `cargo xtask assets verify`: run the checks that were asked for and return
/// the exit code the process should end with.
///
/// `tree` is the directory holding the curated files and `PROVENANCE.md`.
pub fn command(tree: &Path, options: &Options) -> Result<ExitCode, VerifyError> {
    if !options.tree && !options.decode && options.sheet.is_none() {
        return Err(VerifyError::NothingAsked);
    }
    if !tree.is_dir() {
        return Err(VerifyError::Io(format!(
            "no directory at {}: this tool reads the curated tree's own record ({}), and there is \
             no tree there — run it from the repository root",
            tree.display(),
            tree.join(PROVENANCE_FILE).display()
        )));
    }
    let provenance_path = tree.join(PROVENANCE_FILE);
    let text = fs::read_to_string(&provenance_path).map_err(|e| {
        VerifyError::Io(format!(
            "cannot read {}: {e}; the curated tree is checked against its own record, so this \
             tool cannot run without it",
            provenance_path.display()
        ))
    })?;
    let rows = parse_provenance(&text)?;

    // Before anything is written: the tree this tool reads is its input, and a
    // sheet written inside it would become a file with no provenance row.
    if let Some(sheets) = &options.sheet {
        crate::paths::refuse_output_inside(
            sheets,
            tree,
            "--sheet",
            "the curated tree is the input this check reads, and a PNG written among the assets \
             would be a file with no provenance row — the failure --tree exists to report",
            "Pass --sheet somewhere else, e.g. out/assets",
        )
        .map_err(VerifyError::Io)?;
    }

    // Every file in the tree, so a file the record does not list is caught too:
    // each one is supposed to be a copy of something upstream.
    let mut present = Vec::new();
    walk(tree, tree, &mut present)?;
    present.sort();

    let mut findings = 0usize;
    if options.tree {
        findings += check_tree(tree, &rows, &present);
    }
    if options.decode || options.sheet.is_some() {
        findings += check_decode(tree, &rows, options.sheet.as_deref())?;
    }

    if findings > 0 {
        println!("verdict: {findings} finding(s)");
        return Ok(ExitCode::from(1));
    }
    println!("verdict: ok");
    Ok(ExitCode::SUCCESS)
}

/// Every file under `dir`, as paths relative to `root` with `/` separators.
/// `PROVENANCE.md` is the record itself, not one of the files it records.
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), VerifyError> {
    let entries = fs::read_dir(dir)
        .map_err(|e| VerifyError::Io(format!("cannot read {}: {e}", dir.display())))?;
    for entry in entries {
        let entry = entry.map_err(|e| VerifyError::Io(format!("{}: {e}", dir.display())))?;
        let path = entry.path();
        if path.is_dir() {
            walk(root, &path, out)?;
        } else {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if relative != PROVENANCE_FILE {
                out.push(relative);
            }
        }
    }
    Ok(())
}

/// `--tree`: every recorded file hashed, and every file recorded.
fn check_tree(tree: &Path, rows: &[Row], present: &[String]) -> usize {
    let mut findings = 0usize;
    let mut recorded: Vec<&str> = Vec::with_capacity(rows.len());
    for row in rows {
        recorded.push(row.path.as_str());
        let path = tree.join(&row.path);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                findings += 1;
                println!(
                    "FAIL {}: the \"{}\" table records it and the tree has no such file ({error})",
                    row.path, row.table
                );
                continue;
            }
        };
        let actual = crate::reference::sha256_hex(&bytes);
        let size_is_wrong = row.bytes.filter(|recorded| *recorded != bytes.len());
        if actual != row.sha256 {
            findings += 1;
            println!(
                "FAIL {}: the record says sha256 {} and the file is {} ({} bytes)",
                row.path,
                row.sha256,
                actual,
                bytes.len()
            );
        } else if let Some(recorded) = size_is_wrong {
            findings += 1;
            println!(
                "FAIL {}: the record says {recorded} bytes and the file is {}; its digest matches, \
                 so the length in the record is the number that is wrong",
                row.path,
                bytes.len()
            );
        } else {
            println!(
                "ok   {}: {} bytes, sha256 {}",
                row.path,
                bytes.len(),
                &actual[..16]
            );
        }
    }
    for file in present {
        if !recorded.contains(&file.as_str()) {
            findings += 1;
            println!(
                "FAIL {file}: in the tree, and no provenance table records it — every file in \
                 `{DEFAULT_TREE}/` is a copy of something upstream, and this one has no origin in \
                 the record"
            );
        }
    }
    println!(
        "tree: {} file(s) recorded, {} in the tree, {} finding(s)",
        rows.len(),
        present.len(),
        findings
    );
    findings
}

/// `--decode`, and the tiles `--sheet` needs: open every asset, print what it
/// is, and count the ones that refuse after claiming to be a picture.
fn check_decode(tree: &Path, rows: &[Row], sheets: Option<&Path>) -> Result<usize, VerifyError> {
    let mut findings = 0usize;
    let mut pictures = 0usize;
    let mut animations = 0usize;
    let mut palettes = 0usize;
    let mut scenes = 0usize;
    let mut headerless = 0usize;
    let mut refused = 0usize;
    // A recorded refusal that decodes, and a file that cannot be read or inspected: findings,
    // counted so the summary adds up to the number of rows.
    let mut decoded_refusals = 0usize;
    let mut undecodable = 0usize;
    let mut by_part: BTreeMap<String, Vec<Tile>> = BTreeMap::new();

    for row in rows {
        let path = tree.join(&row.path);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) => {
                findings += 1;
                undecodable += 1;
                println!(
                    "FAIL {}: cannot be read, so it cannot be decoded ({error})",
                    row.path
                );
                continue;
            }
        };
        let inspected = match inspect_in_tree(tree, &row.path, &bytes) {
            Ok(inspected) => inspected,
            Err(message) => {
                findings += 1;
                undecodable += 1;
                println!("FAIL {}: {message}", row.path);
                continue;
            }
        };
        match &inspected.kind {
            Kind::Picture(_) => pictures += 1,
            Kind::Frames { .. } => animations += 1,
            Kind::Palette { .. } => palettes += 1,
            Kind::Scene(_) => scenes += 1,
            Kind::Headerless => headerless += 1,
            Kind::Refused { decoded: false, .. } => refused += 1,
            Kind::Refused { decoded: true, .. } => decoded_refusals += 1,
        }
        // A recorded refusal that decodes fails: its digest is unchanged, so `--tree` cannot
        // see a decoder grown lenient.
        if matches!(inspected.kind, Kind::Refused { decoded: true, .. }) {
            findings += 1;
            println!("FAIL {}: {}", row.path, inspected.kind);
        } else {
            println!("ok   {}: {}", row.path, inspected.kind);
        }
        if let (Some(_), Some(tile)) = (sheets, inspected.tile) {
            if row.parts.is_empty() {
                println!(
                    "note {}: the record names no part for this file, so it is on no sheet",
                    row.path
                );
            }
            for part in &row.parts {
                by_part.entry(part.clone()).or_default().push(tile.clone());
            }
        }
    }

    // The two extra categories are printed only when they hold something.
    let mut buckets = String::new();
    if decoded_refusals > 0 {
        buckets.push_str(&format!(
            ", {decoded_refusals} refused on record that now decode"
        ));
    }
    if undecodable > 0 {
        buckets.push_str(&format!(", {undecodable} that could not be decoded"));
    }
    println!(
        "decode: {} asset(s) — {pictures} picture(s), {animations} animation(s), {palettes} \
         palette(s), {scenes} Visu scene file(s), {headerless} headerless with no decoder, \
         {refused} refused on record{buckets}, {findings} finding(s)",
        rows.len()
    );
    if let Some(directory) = sheets {
        write_sheets(directory, &by_part)?;
    }
    Ok(findings)
}

/// One PNG per part, each of that part's assets a tile.
///
/// Parts are written in name order, and the assets of one part in the order the
/// record lists them, so two runs make the same sheets.
fn write_sheets(
    directory: &Path,
    by_part: &BTreeMap<String, Vec<Tile>>,
) -> Result<(), VerifyError> {
    fs::create_dir_all(directory)
        .map_err(|e| VerifyError::Io(format!("cannot create {}: {e}", directory.display())))?;
    for (part, tiles) in by_part {
        let file = directory.join(format!("{}.png", sheet::file_stem(part)));
        sheet::write(&file, tiles)
            .map_err(|e| VerifyError::Io(format!("cannot write {}: {e}", file.display())))?;
    }
    println!(
        "sheets: {} written under {}",
        by_part.len(),
        directory.display()
    );
    Ok(())
}

/// What one asset turned out to be, and the tile it makes on a sheet.
#[derive(Debug)]
struct Inspected {
    kind: Kind,
    tile: Option<Tile>,
}

/// What one asset turned out to be.
#[derive(Debug)]
enum Kind {
    Picture(Picture),
    /// An animation: `count` frames, reported at the first frame's geometry.
    Frames {
        format: Format,
        count: usize,
        width: u32,
        height: u32,
        palette: PaletteUse,
    },
    Palette {
        entries: usize,
    },
    /// One of the vector engine's four packed scene formats. It is data, not a
    /// picture, and its name in `VISU/C/SCENE` is the only thing that says
    /// which of the four it is.
    Scene(Scene),
    /// No magic to go on, and no decoder takes it: a bare screen or mask plane
    /// whose geometry its own part supplies.
    Headerless,
    /// Refused, as the record says it must be. `decoded` is false in every case
    /// this tree has; true means the file decodes after all, and the tree is no
    /// longer the tree the record was written for.
    Refused {
        message: String,
        record: &'static str,
        decoded: bool,
    },
}

#[derive(Debug)]
struct Picture {
    format: Format,
    width: u32,
    height: u32,
    palette: PaletteUse,
}

/// What one file of the vector engine's scene set turned out to be, and the
/// numbers its own reader produced.
///
/// The set has no magic, so the name is the construction: `VISU/C/SCENE` names
/// the tool's output directory (`VISU/C/MAIN.C:243-321`, `SAVE.C:40-90`), and
/// the extension names the piece — `.00M` the master, `.0AA` the scene list,
/// `.0AB` an animation stream and a three-digit object number the object file
/// the scene tool wrote (`U2A.C:148-219`).
#[derive(Debug)]
enum Scene {
    Master {
        /// 0, 1 (`'C'`) or 2 (`'R'`), the city flag of `MAIN.C:290-292`.
        city: u8,
        /// The file's `conum`: the camera is object 0, so this is one more
        /// than the numbers the master lists.
        objects: usize,
    },
    List {
        scenes: usize,
    },
    Animation {
        frames: usize,
        ops: usize,
        /// The first frame's field of vision, the stream's whole run.
        fov: u8,
    },
    Object {
        name: String,
        vertices: usize,
        normals: usize,
        faces: usize,
        lists: usize,
    },
}

impl fmt::Display for Scene {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Scene::Master { city, objects } => write!(
                f,
                "VISU scene master: city {city}, {objects} object(s) (the camera first), palette \
                 256 entries"
            ),
            Scene::List { scenes } => write!(f, "VISU scene list: {scenes} scene(s)"),
            Scene::Animation { frames, ops, fov } => write!(
                f,
                "VISU animation: {frames} frame(s), {ops} command(s), fov 0x{fov:02X}"
            ),
            Scene::Object {
                name,
                vertices,
                normals,
                faces,
                lists,
            } => write!(
                f,
                "VISU object {name}: {vertices} vertices, {normals} normals, {faces} faces, \
                 {lists} lists"
            ),
        }
    }
}

/// How much palette a picture's own file carries. A `.16` file carries 16
/// entries, a `Uh1` container 256, a file with no palette at all none — the
/// count is of the entries the decoder filled with a colour other than black,
/// so it is the file's palette *as loaded*: a 16-colour file whose index 0 is
/// black reads as 15, which is the same number a part drawing with it would
/// see. "None" means every entry is black and the part supplies the palette
/// from somewhere else.
#[derive(Debug)]
enum PaletteUse {
    Entries(usize),
    None,
}

impl fmt::Display for PaletteUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PaletteUse::Entries(count) => write!(f, "palette: {count} of 256 entries set"),
            PaletteUse::None => f.write_str("palette: none set in the file (the part supplies it)"),
        }
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::Picture(picture) => write!(
                f,
                "{} {}x{}, {}",
                picture.format, picture.width, picture.height, picture.palette
            ),
            Kind::Frames {
                format,
                count,
                width,
                height,
                palette,
            } => write!(f, "{format} {count} frames {width}x{height}, {palette}"),
            Kind::Palette { entries } => {
                write!(f, "VGA palette, {entries} entries, six bits per component")
            }
            Kind::Scene(scene) => write!(f, "{scene}"),
            Kind::Headerless => f.write_str(
                "not decoded: no magic to go on, and no decoder takes a headerless file without \
                 the geometry its part knows",
            ),
            Kind::Refused {
                message,
                record,
                decoded,
            } => match decoded {
                false => write!(f, "refused as recorded: {message} ({record})"),
                // A finding, not a note: the file's digest is unchanged when a
                // decoder grows lenient enough to read it, so `--tree` cannot
                // see this and nothing else can either.
                true => write!(
                    f,
                    "decoded, where the record says it must be refused — a refusal that stops \
                     happening is the regression that record exists to catch, and the stale \
                     REFUSED_ON_RECORD entry has to go once the record says otherwise ({record})"
                ),
            },
        }
    }
}

/// Open one asset: by its magic when it has one, by the construction its name
/// names when it has not.
fn inspect(path: &str, bytes: &[u8]) -> Result<Inspected, String> {
    match sniff(bytes) {
        Some(format) => match decode(path, bytes, format) {
            Ok(inspected) => match refused_on_record(path) {
                // The record says this file is the one that must not decode;
                // if it does, the tree changed under the record.
                Some(record) => Ok(Inspected {
                    kind: Kind::Refused {
                        message: String::new(),
                        record,
                        decoded: true,
                    },
                    tile: inspected.tile,
                }),
                None => Ok(inspected),
            },
            Err(error) => match refused_on_record(path) {
                Some(record) => Ok(Inspected {
                    kind: Kind::Refused {
                        message: error.to_string(),
                        record,
                        decoded: false,
                    },
                    tile: Some(byte_view(path, bytes)),
                }),
                None => Err(format!(
                    "{error} — the magic says this is {format} and the decoder refuses it, so it \
                     is a finding. If the refusal is the shipped file's own outcome, it belongs in \
                     REFUSED_ON_RECORD with the record that says so"
                )),
            },
        },
        None => headerless(path, bytes),
    }
}

/// Open one asset, with the tree it came from.
///
/// The scene set is checked first: its animation stream needs its own `.00M`
/// for the object count, and that is a sibling file. Everything else keeps the
/// magic-or-name rule.
fn inspect_in_tree(tree: &Path, path: &str, bytes: &[u8]) -> Result<Inspected, String> {
    match scene_inspected(tree, path, bytes)? {
        Some(inspected) => Ok(inspected),
        None => inspect(path, bytes),
    }
}

/// The directory the scene tool wrote into, relative to the curated tree's
/// root. The vector engine's four formats have no magic at all: this directory
/// and the file's name are what says which one a file is.
const VISU_SCENE_DIR: &str = "VISU/C/SCENE/";

/// Read a `VISU/C/SCENE` file through `sr_assets::visu`, or `None` when the
/// path is not one of them.
///
/// The names are exact, not a guess: `MAIN.C:243-321` writes `<scene>.00M`,
/// `.0AA` and `.0AB`, and `SAVE.C:44` writes `<scene>.%03i` for the objects.
/// A `.0AB` is read with the object count of the `<scene>.00M` beside it,
/// because that is where the part gets it (`U2A.C:168-172`); a missing master
/// is a finding for the animation, not a reason to guess a count.
fn scene_inspected(tree: &Path, path: &str, bytes: &[u8]) -> Result<Option<Inspected>, String> {
    let Some(name) = path.strip_prefix(VISU_SCENE_DIR) else {
        return Ok(None);
    };
    // Files of the set sit directly in the directory; a subdirectory's file
    // is not one of the tool's outputs.
    if name.contains('/') {
        return Ok(None);
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return Ok(None);
    };
    let extension = extension.to_ascii_lowercase();
    let is_object = extension.len() == 3
        && extension
            .chars()
            .all(|character| character.is_ascii_digit());

    let scene = match extension.as_str() {
        "00m" => {
            let master = read_master(bytes).map_err(|error| {
                format!("its name says VISU scene master, so it has to be one: {error}")
            })?;
            Scene::Master {
                city: master.city,
                // The file holds conum-1 numbers; object 0 is the camera.
                objects: master.objects.len() + 1,
            }
        }
        "0aa" => {
            let list = read_scene_list(bytes).map_err(|error| {
                format!("its name says VISU scene list, so it has to be one: {error}")
            })?;
            Scene::List {
                scenes: list.scenes.len(),
            }
        }
        "0ab" => {
            let master_path = tree.join(VISU_SCENE_DIR).join(format!("{stem}.00M"));
            let master_bytes = fs::read(&master_path).map_err(|error| {
                format!(
                    "its name says VISU animation, and the object count it is decoded with is in \
                     {}: {error}",
                    master_path.display()
                )
            })?;
            let master = read_master(&master_bytes)
                .map_err(|error| format!("{}: {error}", master_path.display()))?;
            let animation = read_animation(bytes, master.objects.len() + 1).map_err(|error| {
                format!("its name says VISU animation, so it has to be one: {error}")
            })?;
            Scene::Animation {
                frames: animation.frames.len(),
                ops: animation.frames.iter().map(|frame| frame.ops.len()).sum(),
                fov: animation.frames.first().map_or(0, |frame| frame.fov),
            }
        }
        _ if is_object => {
            let object = read_object(bytes).map_err(|error| {
                format!("its name says VISU object, so it has to be one: {error}")
            })?;
            Scene::Object {
                name: String::from_utf8_lossy(&object.name).into_owned(),
                vertices: object.vertices.len(),
                normals: object.normals.len(),
                faces: object.polys.len(),
                lists: object.lists.len(),
            }
        }
        _ => return Ok(None),
    };

    Ok(Some(Inspected {
        kind: Kind::Scene(scene),
        // Scene data is not a picture. Its bytes still make a tile so the
        // sheets keep one per curated file, but the label says what it is.
        tile: Some(bytes_tile(
            path,
            bytes,
            "VISU scene data, not a picture; the bytes are shown as grey",
        )),
    }))
}

/// The formats with a magic: a picture or an animation, and either way the
/// decoder has to read it.
///
/// Errors are messages rather than `AssetError`s because the tool only ever
/// prints them, and because "the decoder refused it" and "the decoder returned
/// nothing" are the same kind of outcome here: a finding.
fn decode(path: &str, bytes: &[u8], format: Format) -> Result<Inspected, String> {
    match format {
        Format::Fli => {
            let frames = load_frames(bytes, format).map_err(|e| e.to_string())?;
            frames_inspected(path, format, frames)
        }
        _ => {
            let image = load(bytes, format).map_err(|e| e.to_string())?;
            Ok(picture_inspected(path, format, image))
        }
    }
}

fn picture_inspected(path: &str, format: Format, image: Image) -> Inspected {
    let palette = palette_use(&image.palette);
    let (width, height) = (u32::from(image.width()), u32::from(image.height()));
    let (rgb, note) = match palette {
        PaletteUse::Entries(_) => (
            through_palette(image.pixels(), &image.palette),
            String::new(),
        ),
        // Every entry black: the file carries no palette, so show the indices
        // rather than a black rectangle nobody can judge.
        PaletteUse::None => (
            through_indices(image.pixels()),
            " — indices, because the file carries no palette".to_owned(),
        ),
    };
    Inspected {
        kind: Kind::Picture(Picture {
            format,
            width,
            height,
            palette,
        }),
        tile: Some(Tile {
            width,
            height,
            rgb,
            label: format!("{path} ({format} {width}x{height}){note}"),
        }),
    }
}

/// An animation: `count` frames, reported at the first frame's geometry.
///
/// A container that holds *no* frames is a finding, not a panic: `decode_fli` on
/// a header-only file and `decode_ani` on empty bytes both return an empty list,
/// and a tool whose contract is report-don't-panic reports it.
fn frames_inspected(path: &str, format: Format, frames: Vec<Image>) -> Result<Inspected, String> {
    let count = frames.len();
    let Some(first) = frames.into_iter().next() else {
        return Err(format!(
            "{format} holds no frames at all — the container parsed and its frame list is empty, \
             so either it is not an animation or it is a stub of one"
        ));
    };
    let palette = palette_use(&first.palette);
    let (width, height) = (u32::from(first.width()), u32::from(first.height()));
    let rgb = match palette {
        PaletteUse::Entries(_) => through_palette(first.pixels(), &first.palette),
        PaletteUse::None => through_indices(first.pixels()),
    };
    Ok(Inspected {
        kind: Kind::Frames {
            format,
            count,
            width,
            height,
            palette,
        },
        tile: Some(Tile {
            width,
            height,
            rgb,
            label: format!("{path} ({format} frame 1 of {count}, {width}x{height})"),
        }),
    })
}

/// A file with no magic. `.PAL` and `.ANI` are the two the crate documents as
/// "loaded by construction", so their names are their door and what follows must
/// hold; everything else is reported, not guessed at.
fn headerless(path: &str, bytes: &[u8]) -> Result<Inspected, String> {
    match extension(path).as_str() {
        "pal" => {
            let palette = pal::decode(bytes)
                .map_err(|e| format!("its name says palette, so it has to be one: {e}"))?;
            let mut rgb = Vec::with_capacity(256 * 3 * PALETTE_STRIP_HEIGHT as usize);
            for _ in 0..PALETTE_STRIP_HEIGHT {
                for index in 0..=255u8 {
                    rgb.extend_from_slice(&expanded(&palette, index));
                }
            }
            Ok(Inspected {
                kind: Kind::Palette { entries: 256 },
                tile: Some(Tile {
                    width: 256,
                    height: PALETTE_STRIP_HEIGHT,
                    rgb,
                    label: format!("{path} (palette strip, 256 entries)"),
                }),
            })
        }
        "ani" => {
            let frames = load_frames(bytes, Format::Ani)
                .map_err(|e| format!("its name says ANI, so it has to be one: {e}"))?;
            frames_inspected(path, Format::Ani, frames)
                .map_err(|e| format!("its name says ANI, so it has to be one: {e}"))
        }
        _ => Ok(Inspected {
            kind: Kind::Headerless,
            tile: Some(byte_view(path, bytes)),
        }),
    }
}

/// The record entry for a file whose refusal is the recorded outcome.
fn refused_on_record(path: &str) -> Option<&'static str> {
    REFUSED_ON_RECORD
        .iter()
        .find(|(recorded, _)| *recorded == path)
        .map(|(_, record)| *record)
}

/// The extension, lowercased, without the dot.
fn extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|extension| extension.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

/// How many of the 256 entries the decoder filled with a colour.
fn palette_use(palette: &Palette) -> PaletteUse {
    let entries = (0..=255u8)
        .filter(|index| palette.entry(*index) != [0, 0, 0])
        .count();
    if entries == 0 {
        PaletteUse::None
    } else {
        PaletteUse::Entries(entries)
    }
}

fn expanded(palette: &Palette, index: u8) -> [u8; 3] {
    let entry = palette.entry(index);
    [
        dac_expand(entry[0]),
        dac_expand(entry[1]),
        dac_expand(entry[2]),
    ]
}

/// Indices through their palette, widened by the engine's own rule — the
/// picture as the part draws it.
fn through_palette(pixels: &[u8], palette: &Palette) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for &index in pixels {
        rgb.extend_from_slice(&expanded(palette, index));
    }
    rgb
}

/// Indices as a grey ramp, for a file that carries no palette: the structure is
/// visible even though the colours belong to the part.
fn through_indices(pixels: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(pixels.len() * 3);
    for &index in pixels {
        rgb.extend_from_slice(&[index, index, index]);
    }
    rgb
}

/// A file with no decoder, at the width a screen is: its first bytes as grey.
/// The label says it is bytes, because that is what it is.
fn byte_view(path: &str, bytes: &[u8]) -> Tile {
    bytes_tile(path, bytes, "no decoder takes this file")
}

/// The same view for a file a decoder *does* take but which is not a picture:
/// the label says what the bytes are instead of claiming nothing reads them.
fn bytes_tile(path: &str, bytes: &[u8], note: &str) -> Tile {
    let width = BYTE_VIEW_WIDTH.min(bytes.len().max(1));
    let height = bytes.len().div_ceil(width).clamp(1, BYTE_VIEW_ROWS);
    let shown = (width * height).min(bytes.len());
    let mut rgb = vec![0u8; width * height * 3];
    for (index, byte) in bytes[..shown].iter().enumerate() {
        rgb[index * 3] = *byte;
        rgb[index * 3 + 1] = *byte;
        rgb[index * 3 + 2] = *byte;
    }
    let cut = if shown < bytes.len() {
        format!(", first {shown} of {} bytes", bytes.len())
    } else {
        String::new()
    };
    Tile {
        width: width as u32,
        height: height as u32,
        rgb,
        label: format!("{path} (bytes at {width} wide{cut}: {note})"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real record, read at compile time: these tests are about the file
    /// this tool actually reads, not about an example of one.
    const PROVENANCE: &str = include_str!("../../assets/PROVENANCE.md");

    fn digest(character: char) -> String {
        character.to_string().repeat(64)
    }

    #[test]
    fn the_real_record_reads_two_tables_and_only_the_provenance_ones() {
        let rows = parse_provenance(PROVENANCE).unwrap();

        // A1's main table and A5's recovered table, and nothing else: the
        // *Collapsed duplicates* table's `dropped path` column and the
        // *Identical files that are not copied at all* table's `path(s)` column
        // are records **about** the tree, not rows of it.
        assert_eq!(
            rows.len(),
            179,
            "178 curated files (twenty of them the shipped part EXEs, D49) and A5's one recovered file"
        );
        assert_eq!(rows[0].path, "ALKU/FONA.UH");
        assert_eq!(rows[0].parts, vec!["ALKU".to_string()]);
        assert_eq!(
            rows[0].sha256,
            "b17f1487c885ce0f9254a9b7f98316d17c27448d7348b734bf3539bbcbbb0522"
        );

        // The recovered file is the last row, in its own table, and that table
        // carries the same `parts` column the main one does — so the one asset
        // with no source art beside it is on its part's sheet like every other
        // asset, which is the asset a person most needs to see there.
        let recovered = rows.last().unwrap();
        assert_eq!(recovered.path, "TECHNO/CIRCLE2.EGA");
        assert_eq!(recovered.table, "Recovered assets");
        assert_eq!(recovered.parts, vec!["TECHNO".to_string()]);
        assert!(
            recovered.bytes.is_none(),
            "that table records no `bytes` column"
        );

        // The three tables that are not provenance contribute no rows.
        for path in ["MAIN/DATA/O2.SCI", "BEG/TMP.BBM", "END/NUTS.LBM"] {
            assert!(
                !rows.iter().any(|row| row.path == path),
                "{path} is in a table that is not the provenance"
            );
        }
    }

    #[test]
    fn parts_are_read_from_the_column_that_names_them() {
        let rows = parse_provenance(
            "| path | sha256 | parts |\n|---|---|---|\n| `a/b.UH` | `11` | `U2A, U2E` |\n"
                .replace("`11`", &format!("`{}`", digest('1')))
                .as_str(),
        )
        .unwrap();

        assert_eq!(rows[0].parts, vec!["U2A".to_string(), "U2E".to_string()]);
    }

    #[test]
    fn a_row_that_does_not_fit_its_table_is_malformed() {
        // Three columns of header, two cells of row: the record cannot say what
        // this file's digest was, so there is nothing to check against.
        let text = "| path | origin | sha256 |\n|---|---|---|\n| `a.UH` | `upstream/a.UH` |\n";
        let error = parse_provenance(text).unwrap_err();
        assert!(matches!(error, VerifyError::Malformed(_)), "{error}");
    }

    #[test]
    fn a_digest_that_is_not_a_digest_is_malformed() {
        // A short hex string, a long one, and something that is not hex at all:
        // a record that cannot say what the file's digest was is not a record
        // this tool can check anything against.
        for bad in [
            "b17f1487",
            &digest('0').repeat(2),
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz",
        ] {
            let text = format!("| path | sha256 |\n|---|---|\n| `a.UH` | `{bad}` |\n");
            let error = parse_provenance(&text).unwrap_err();
            assert!(matches!(error, VerifyError::Malformed(_)), "{bad}: {error}");
        }
    }

    #[test]
    fn one_path_in_two_tables_is_malformed_however_far_apart_they_are() {
        // The curated and the recovered tables are two tables in one file, and
        // a path in both would be hashed and decoded twice. Adjacent rows are
        // the easy half of that; the rows here are not adjacent at all.
        let text = format!(
            "## Files\n\n| path | sha256 |\n|---|---|\n| `a.UH` | `{}` |\n\n## Recovered assets\n\n\
             | path | blob | sha256 |\n|---|---|---|\n| `a.UH` | `x.OBK` | `{}` |\n",
            digest('1'),
            digest('2'),
        );
        let error = parse_provenance(&text).unwrap_err();
        assert!(matches!(error, VerifyError::Malformed(_)), "{error}");
        let message = error.to_string();
        assert!(
            message.contains("Files") && message.contains("Recovered assets"),
            "{message}"
        );
    }

    #[test]
    fn nothing_asked_for_is_a_tool_failure_not_a_pass() {
        // A run that checks nothing cannot report that everything checked out.
        let tree =
            std::env::temp_dir().join(format!("xtask-assets-nothing-{}", std::process::id()));
        let error = command(&tree, &Options::default()).unwrap_err();
        assert!(matches!(error, VerifyError::NothingAsked), "{error}");
    }

    /// The curated tree, as the tool sees it when run from `xtask/`.
    fn asset(path: &str) -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../assets")
                .join(path),
        )
        .unwrap_or_else(|e| panic!("cannot read assets/{path}: {e}"))
    }

    /// The root those files are read from.
    fn asset_tree() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../assets")
    }

    #[test]
    fn a_picture_is_opened_by_its_magic_and_reported_with_its_palette() {
        let inspected = inspect("ALKU/FONA.UH", &asset("ALKU/FONA.UH")).unwrap();

        let Kind::Picture(picture) = &inspected.kind else {
            panic!("{}", inspected.kind);
        };
        assert_eq!(picture.format, Format::Origin);
        // 1500x30, the strip A7 reads its 76 glyph cells out of.
        assert_eq!((picture.width, picture.height), (1500, 30));
        assert!(matches!(picture.palette, PaletteUse::Entries(_)));
        // A strip is a mask: its four values are not a picture, but they are
        // what the file holds, and the tile is that.
        let tile = inspected.tile.unwrap();
        assert_eq!((tile.width, tile.height), (1500, 30));
        assert_eq!(tile.rgb.len(), 1500 * 30 * 3);
    }

    #[test]
    fn a_file_with_no_palette_is_shown_as_its_indices() {
        // `COMAN/COMBG.X` carries 16 bytes of header and 64,000 pixels of
        // 320x200 screen, and its palette region is all zeros — the part sets
        // the colours (asset-inventory §E.2). Drawn through the palette it has,
        // it would be a black rectangle.
        let inspected = inspect("COMAN/COMBG.X", &asset("COMAN/COMBG.X")).unwrap();

        let Kind::Picture(picture) = &inspected.kind else {
            panic!("{}", inspected.kind);
        };
        assert_eq!((picture.width, picture.height), (320, 200));
        assert!(matches!(picture.palette, PaletteUse::None));
        let tile = inspected.tile.unwrap();
        assert!(tile.label.contains("indices"), "{}", tile.label);
        // Not all one colour: the indices are the picture's structure.
        assert!(tile.rgb.chunks_exact(3).any(|pixel| pixel[0] != 0));
    }

    #[test]
    fn an_animation_is_opened_by_its_name_and_counted() {
        // `.ANI` has no magic; `sr_assets` documents it as loaded by
        // construction, so its name is the door — and A6 measured 42 frames.
        let inspected = inspect("PAM/OUT.ANI", &asset("PAM/OUT.ANI")).unwrap();

        let Kind::Frames {
            format,
            count,
            width,
            height,
            ..
        } = &inspected.kind
        else {
            panic!("{}", inspected.kind);
        };
        assert_eq!(*format, Format::Ani);
        assert_eq!(*count, 42);
        assert_eq!((*width, *height), (320, 200));
    }

    #[test]
    fn a_name_that_promises_a_palette_and_is_not_one_is_a_finding() {
        // 768 bytes of six-bit triples is what the five `.PAL` files in the
        // tree are; a `.PAL` of any other length is the name lying.
        let short = vec![0u8; 767];
        let error = inspect("SOMEWHERE/PAL.PAL", &short).unwrap_err();
        assert!(error.contains("palette"), "{error}");

        // And eight-bit values are the other half of the same contract.
        let mut eight_bit = vec![0u8; 768];
        eight_bit[0] = 255;
        let error = inspect("SOMEWHERE/PAL.PAL", &eight_bit).unwrap_err();
        assert!(error.contains("palette"), "{error}");
    }

    #[test]
    fn a_headerless_file_is_reported_and_not_guessed_at() {
        // `PANIC/MONSTER.U` is a 320x200 screen with no header at all. Its
        // geometry is the part's, and a tool that read it at 250x256 because
        // the arithmetic allowed it would be inventing the one thing the file
        // does not say.
        let inspected = inspect("PANIC/MONSTER.U", &asset("PANIC/MONSTER.U")).unwrap();

        assert!(
            matches!(inspected.kind, Kind::Headerless),
            "{}",
            inspected.kind
        );
        assert!(inspected.kind.to_string().contains("no magic"));
        let tile = inspected.tile.unwrap();
        // The byte view is the demo's screen width, so a bare screen lands at
        // its own geometry rather than somewhere arbitrary.
        assert_eq!((tile.width, tile.height), (320, 200));
    }

    #[test]
    fn a_visu_scene_file_is_read_by_its_name_and_reported() {
        // The four packed scene formats have no magic: `VISU/C/SCENE/` and
        // `<scene>.{00M,0AA,0AB,NNN}` are the construction the scene tool's
        // own writer gives them. Each one's own reader produces the numbers.
        let tree = asset_tree();

        let master = inspect_in_tree(
            &tree,
            "VISU/C/SCENE/U2A.00M",
            &asset("VISU/C/SCENE/U2A.00M"),
        )
        .unwrap();
        let Kind::Scene(Scene::Master { city, objects }) = master.kind else {
            panic!("{}", master.kind);
        };
        assert_eq!((city, objects), (0, 6), "the camera is object 0");

        let list = inspect_in_tree(
            &tree,
            "VISU/C/SCENE/U2A.0AA",
            &asset("VISU/C/SCENE/U2A.0AA"),
        )
        .unwrap();
        let Kind::Scene(Scene::List { scenes }) = list.kind else {
            panic!("{}", list.kind);
        };
        assert_eq!(scenes, 1);

        let animation = inspect_in_tree(
            &tree,
            "VISU/C/SCENE/U2A.0AB",
            &asset("VISU/C/SCENE/U2A.0AB"),
        )
        .unwrap();
        let Kind::Scene(Scene::Animation { frames, ops, fov }) = animation.kind else {
            panic!("{}", animation.kind);
        };
        assert_eq!((frames, ops, fov), (522, 1272, 0x22));

        let object = inspect_in_tree(
            &tree,
            "VISU/C/SCENE/U2A.001",
            &asset("VISU/C/SCENE/U2A.001"),
        )
        .unwrap();
        let Kind::Scene(Scene::Object {
            name,
            vertices,
            normals,
            faces,
            lists,
        }) = &object.kind
        else {
            panic!("{}", object.kind);
        };
        assert_eq!(name, "\"s01\"");
        assert_eq!((*vertices, *normals, *faces, *lists), (159, 269, 124, 9));

        // Scene data is not a picture, but it still makes a tile so the
        // per-part sheets hold one per curated file; the label says what the
        // bytes are instead of claiming nothing reads them.
        let tile = object.tile.unwrap();
        assert!(
            tile.label.contains("VISU scene data, not a picture"),
            "{}",
            tile.label
        );
    }

    #[test]
    fn a_scene_name_that_lies_is_a_finding_and_a_scene_name_outside_the_directory_is_not_one() {
        let tree = asset_tree();
        let error = inspect_in_tree(&tree, "VISU/C/SCENE/U2A.001", b"not an object").unwrap_err();
        assert!(error.contains("VISU object"), "{error}");

        // The `.0AB` count comes from its `.00M`: the master is read first,
        // and the animation's own bytes are then refused.
        let error = inspect_in_tree(&tree, "VISU/C/SCENE/U2A.0AB", &[0u8; 4]).unwrap_err();
        assert!(error.contains("VISU animation"), "{error}");

        // A `.001` somewhere else is not the scene tool's object: the
        // directory is part of the construction, so the generic headerless
        // report stands.
        let inspected = inspect_in_tree(&tree, "SOMEWHERE/U2A.001", b"whatever").unwrap();
        assert!(
            matches!(inspected.kind, Kind::Headerless),
            "{}",
            inspected.kind
        );
    }

    #[test]
    fn a_picture_the_magic_promises_and_the_decoder_refuses_is_a_finding() {
        // An `FC FC` header claiming 320x200 with no pixels behind it. Nothing
        // recorded says this file is allowed to fail, so it does not pass.
        let mut bytes = vec![0u8; 784];
        bytes[0] = 0xFC;
        bytes[1] = 0xFC;
        let error = inspect("NOT/RECORDED.UH", &bytes).unwrap_err();
        assert!(error.contains("REFUSED_ON_RECORD"), "{error}");
    }

    #[test]
    fn a_container_that_holds_no_frames_is_a_finding_not_a_panic() {
        // `decode_ani` on empty bytes and `decode_fli` on a header with no frame
        // records both return an empty list. This tool's contract is
        // report-don't-panic, so a degenerate container is reported like any
        // other unreadable asset — exit 1, not exit 101.
        let error = inspect("SOMEWHERE/EMPTY.ANI", b"").unwrap_err();
        assert!(error.contains("no frames at all"), "{error}");

        let mut header_only = vec![0u8; 128];
        header_only[4] = 0x11;
        header_only[5] = 0xAF;
        header_only[8..10].copy_from_slice(&320u16.to_le_bytes());
        header_only[10..12].copy_from_slice(&200u16.to_le_bytes());
        let error = inspect("SOMEWHERE/EMPTY.FLI", &header_only).unwrap_err();
        assert!(error.contains("no frames at all"), "{error}");
    }

    #[test]
    fn the_recorded_refusal_is_the_curated_tree_without_exceptions() {
        // The list is one entry, and the check's strength depends on it staying
        // that way: every other magic-carrying file in the tree decodes.
        assert_eq!(REFUSED_ON_RECORD.len(), 1);
        assert_eq!(REFUSED_ON_RECORD[0].0, "COMAN/COMBG.UH");

        let refused = inspect("COMAN/COMBG.UH", &asset("COMAN/COMBG.UH")).unwrap();
        let Kind::Refused {
            decoded, message, ..
        } = &refused.kind
        else {
            panic!("{}", refused.kind);
        };
        assert!(!decoded);
        assert!(message.contains("64016"), "{message}");
    }
}
