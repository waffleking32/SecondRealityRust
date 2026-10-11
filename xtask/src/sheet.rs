//! Contact sheets: many tiles, one PNG.
//!
//! Two callers, one layout. `assets verify --sheet` puts every asset a part
//! needs on one page, and `reference capture --sheet` puts the frames a capture
//! just took on one page — B2–B5 identify a part's frames by *looking* at a
//! sweep, so the sheet a capture writes and the sheet the asset check writes
//! have to be the same picture-language. [`contact_sheet`] is the only place a
//! sheet is laid out and [`write_png`] the only place one is written; a second
//! layout is a second thing to learn to read.
//!
//! Three properties are deliberate:
//!
//! - **Nothing is scaled or cropped.** A tile is drawn at its own size. A 1500×30
//!   font strip and a 320×200 background look like what they are, and a scaled
//!   tile would make "is this the picture the part shows" a question about the
//!   scaling. Cells are uniform — the widest and tallest tile set the cell — so
//!   a small tile sits in a corner of its cell instead of pushing its
//!   neighbours around.
//! - **Every cell is a panel.** Each cell of the grid is filled with a light
//!   colour and the tile is drawn at its top-left corner, with a darker gutter
//!   between the cells. A picture's edge is visible whatever the picture holds
//!   — a black tile on a black background is a tile nobody can find — and a
//!   partially filled last row shows as empty panels, so a person counting
//!   tiles counts cells rather than guessing where the row ended.
//! - **[`Tile::label`] is never drawn.** The `png` crate has no text and the
//!   tree has no font — and this project does not carry a bitmap-font renderer
//!   for a debug picture (A7 dropped `GRAB/BLOWFNT.1BM` as a *font*, so there
//!   is not even a strip to borrow glyphs from). The label is what the caller
//!   prints for each tile's position instead: a sheet whose tiles are named
//!   nowhere is a sheet nobody can read.
//!
//! Grid order is row-major: tile `i` is at row `i / columns`, column
//! `i % columns`, and that is what the callers print against each label.

use std::fs;
use std::io::{self, BufWriter};
use std::path::Path;

/// One picture on a sheet: RGB8, at its own size, and what it is called.
///
/// `Clone` because a tile can belong to two parts — one asset, two sheets — and
/// a sheet is a debug picture, not a place to be clever about sharing pixels.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub width: u32,
    pub height: u32,
    /// `width * height * 3` bytes, row-major, no padding.
    pub rgb: Vec<u8>,
    /// This tile's name, for the caller's console output and file naming. It is
    /// metadata: nothing draws it.
    pub label: String,
}

/// The colour of the sheet between the cells, and of its outer border. Darker
/// than a cell, so the grid reads as a grid.
const GUTTER_COLOUR: [u8; 3] = [40, 40, 40];

/// The panel each tile sits on. A tile is drawn at its cell's top-left corner
/// and the rest of the panel shows, so a picture's edge is visible whatever the
/// picture holds — a black tile on a black background is a tile nobody can
/// find — and a partially filled last row is countable.
const CELL_COLOUR: [u8; 3] = [120, 120, 120];

/// Background between one cell and the next.
const GUTTER: u32 = 2;

/// The outline's thickness.
const BORDER: u32 = 1;

/// Lay `tiles` out in a grid of `columns` and return `(width, height, rgb)`.
///
/// `columns` is clamped to `1..=tiles.len()`: a sheet of no columns would have
/// no width, and asking for more columns than tiles is asking for empty
/// columns. A sheet of no tiles has no pixels at all, and [`write_png`] refuses
/// it — a 0×0 PNG is not a picture.
pub fn contact_sheet(tiles: &[Tile], columns: usize) -> (u32, u32, Vec<u8>) {
    if tiles.is_empty() {
        return (0, 0, Vec::new());
    }
    let columns = clamp_columns(tiles.len(), columns);
    let rows = tiles.len().div_ceil(columns);
    let (cell_width, cell_height) = tiles.iter().fold((0, 0), |(w, h), tile| {
        (w.max(tile.width), h.max(tile.height))
    });
    let pitch_x = cell_width + 2 * BORDER + GUTTER;
    let pitch_y = cell_height + 2 * BORDER + GUTTER;
    let width = GUTTER + columns as u32 * pitch_x;
    let height = GUTTER + rows as u32 * pitch_y;

    let mut rgb = vec![0u8; width as usize * height as usize * 3];
    fill(&mut rgb, width as usize, GUTTER_COLOUR);
    // Every cell in the grid gets its outline, including the ones a partial
    // last row leaves empty: a person counting tiles counts cells, and an empty
    // cell that is not drawn is a cell they cannot count.
    for index in 0..rows * columns {
        let (row, column) = (index / columns, index % columns);
        let x = GUTTER + column as u32 * pitch_x;
        let y = GUTTER + row as u32 * pitch_y;
        fill_rect(
            &mut rgb,
            width as usize,
            x,
            y,
            cell_width + 2 * BORDER,
            cell_height + 2 * BORDER,
            CELL_COLOUR,
        );
    }
    for (index, tile) in tiles.iter().enumerate() {
        let (row, column) = (index / columns, index % columns);
        let x = GUTTER + column as u32 * pitch_x;
        let y = GUTTER + row as u32 * pitch_y;
        draw_tile(&mut rgb, width as usize, tile, x + BORDER, y + BORDER);
    }
    (width, height, rgb)
}

/// How many columns a sheet of `count` tiles should have by default: the
/// squarest grid, which keeps a sheet of frames or assets roughly the shape of
/// a screen rather than a strip that scrolls for a metre.
pub fn columns_for(count: usize) -> usize {
    if count == 0 {
        return 1;
    }
    let mut columns = 1;
    while columns * columns < count {
        columns += 1;
    }
    columns
}

/// `columns` as a grid of `count` tiles can use it.
pub fn clamp_columns(count: usize, columns: usize) -> usize {
    columns.clamp(1, count.max(1))
}

/// The tile position a caller prints against a label: `(row, column)`.
pub fn position(index: usize, columns: usize) -> (usize, usize) {
    (index / columns, index % columns)
}

/// A part's name as a file name for its sheet. ASCII letters, digits, `.`, `_`
/// and `-` are kept — and so is the `&` in `LNS&ZOOM`, which is the part's own
/// name and reads as one on every filesystem this project runs on. Everything
/// else becomes `_`, so a name from the record can never point at a different
/// directory than the one the caller named.
pub fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '&') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "sheet".to_owned()
    } else {
        stem
    }
}

/// Write `rgb` as an 8-bit RGB PNG.
///
/// The writer `compare`'s diff images already use, so a sheet is a PNG the same
/// tools open.
pub fn write_png(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
    if width == 0 || height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{width}x{height} is not a picture; there is nothing to write"),
        ));
    }
    let expected = width as usize * height as usize * 3;
    if rgb.len() != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{width}x{height} RGB8 needs {expected} bytes, {} given",
                rgb.len()
            ),
        ));
    }
    let file = fs::File::create(path)?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header()?;
    writer.write_image_data(rgb)?;
    writer.finish()?;
    Ok(())
}

fn draw_tile(dst: &mut [u8], dst_width: usize, tile: &Tile, x: u32, y: u32) {
    // A tile whose buffer is shorter than its stated geometry is drawn as far
    // as it goes: the callers build both together, and a sheet is a debug
    // picture — refusing to write it over a bookkeeping error would lose the
    // picture, not fix the error.
    let row_bytes = tile.width as usize * 3;
    for row in 0..tile.height {
        let source = row as usize * row_bytes;
        if source + row_bytes > tile.rgb.len() {
            break;
        }
        let target = ((y + row) as usize * dst_width + x as usize) * 3;
        if target + row_bytes > dst.len() {
            break;
        }
        dst[target..target + row_bytes].copy_from_slice(&tile.rgb[source..source + row_bytes]);
    }
}

fn fill(dst: &mut [u8], dst_width: usize, colour: [u8; 3]) {
    fill_rect(
        dst,
        dst_width,
        0,
        0,
        dst_width as u32,
        (dst.len() / 3 / dst_width) as u32,
        colour,
    );
}

fn fill_rect(
    dst: &mut [u8],
    dst_width: usize,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    colour: [u8; 3],
) {
    for row in y..y + height {
        let start = ((row as usize * dst_width) + x as usize) * 3;
        let end = start + width as usize * 3;
        if end > dst.len() {
            break;
        }
        for pixel in dst[start..end].chunks_exact_mut(3) {
            pixel.copy_from_slice(&colour);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(width: u32, height: u32, value: u8, label: &str) -> Tile {
        Tile {
            width,
            height,
            rgb: vec![value; width as usize * height as usize * 3],
            label: label.to_owned(),
        }
    }

    fn pixel(rgb: &[u8], width: u32, x: u32, y: u32) -> [u8; 3] {
        let at = ((y * width + x) * 3) as usize;
        [rgb[at], rgb[at + 1], rgb[at + 2]]
    }

    /// Where a cell starts, for a sheet whose cells are `cell` big: `(x, y)` of
    /// the cell's outline.
    fn cell_origin(cell: (u32, u32), column: usize, row: usize) -> (u32, u32) {
        let pitch_x = cell.0 + 2 * BORDER + GUTTER;
        let pitch_y = cell.1 + 2 * BORDER + GUTTER;
        (
            GUTTER + column as u32 * pitch_x,
            GUTTER + row as u32 * pitch_y,
        )
    }

    #[test]
    fn tiles_are_laid_out_row_major_with_a_gutter_between_them() {
        // Four 4x2 tiles in two columns: a 2x2 grid. A cell is 4+2*1 wide and
        // 2+2*1 tall, with a 2-pixel gutter around and between the cells.
        let tiles = [
            tile(4, 2, 0x11, "a"),
            tile(4, 2, 0x22, "b"),
            tile(4, 2, 0x33, "c"),
            tile(4, 2, 0x44, "d"),
        ];
        let (width, height, rgb) = contact_sheet(&tiles, 2);

        assert_eq!((width, height), (2 + 2 * (4 + 2 + 2), 2 + 2 * (2 + 2 + 2)));
        // The background, at the top-left corner.
        assert_eq!(pixel(&rgb, width, 0, 0), GUTTER_COLOUR);
        for (index, value) in [(0, 0x11), (1, 0x22), (2, 0x33), (3, 0x44)] {
            let (x, y) = cell_origin((4, 2), index % 2, index / 2);
            // The outline of the cell, then the tile's own first pixel inside
            // it: drawn at its own size, in the cell's top-left corner.
            assert_eq!(pixel(&rgb, width, x, y), CELL_COLOUR, "cell {index}");
            assert_eq!(
                pixel(&rgb, width, x + BORDER, y + BORDER),
                [value; 3],
                "cell {index}"
            );
        }
        // The gutter between two cells is the background, in both directions.
        assert_eq!(pixel(&rgb, width, 8, 3), GUTTER_COLOUR);
        assert_eq!(pixel(&rgb, width, 3, 6), GUTTER_COLOUR);
    }

    #[test]
    fn the_largest_tile_sets_every_cell() {
        // A small tile in a cell sized by a big one: drawn at its own size, in
        // the cell's corner, with the panel showing around it — not stretched.
        let tiles = [tile(2, 1, 0x77, "small"), tile(4, 3, 0x88, "big")];
        let (width, height, rgb) = contact_sheet(&tiles, 2);

        assert_eq!((width, height), (2 + 2 * 8, 2 + 3 + 2 + 2));
        // The small tile is 2x1: both its columns are its pixels, and one
        // column to the right of it the cell's panel shows again.
        assert_eq!(pixel(&rgb, width, 3, 3), [0x77; 3]);
        assert_eq!(pixel(&rgb, width, 4, 3), [0x77; 3]);
        assert_eq!(pixel(&rgb, width, 5, 3), CELL_COLOUR);
        assert_eq!(pixel(&rgb, width, 3, 4), CELL_COLOUR);
        // The gutter between the two cells is the sheet's darker background.
        assert_eq!(pixel(&rgb, width, 9, 3), GUTTER_COLOUR);
        assert_eq!(pixel(&rgb, width, 11, 4), [0x88; 3]);
    }

    #[test]
    fn a_trailing_partial_row_still_gets_its_own_cells() {
        // Six tiles in four columns: two rows, the second half empty. The
        // empty cells are drawn as outlines, so a reader can count them.
        let tiles: Vec<Tile> = (0..6).map(|_| tile(2, 2, 0x10, "t")).collect();
        let (width, height, rgb) = contact_sheet(&tiles, 4);

        assert_eq!((width, height), (2 + 4 * 6, 2 + 2 * 6));
        // The sixth tile is at row 1, column 1; the two cells after it hold no
        // tile at all, so they are drawn as their outlines and nothing else —
        // a reader can count the empty cells rather than losing their place.
        assert_eq!(pixel(&rgb, width, 9, 9), [0x10; 3]);
        let (x, y) = cell_origin((2, 2), 2, 1);
        assert_eq!(pixel(&rgb, width, x, y), CELL_COLOUR);
        assert_eq!(pixel(&rgb, width, x + 3, y + 3), CELL_COLOUR);
        assert_eq!(pixel(&rgb, width, x - 1, y), GUTTER_COLOUR);
    }

    #[test]
    fn columns_are_clamped_to_the_tiles_that_exist() {
        let tiles: Vec<Tile> = (0..3).map(|_| tile(1, 1, 1, "t")).collect();

        // Zero columns would be a sheet of no width; more columns than tiles
        // would be columns nobody can see.
        assert_eq!(contact_sheet(&tiles, 0).0, contact_sheet(&tiles, 1).0);
        assert_eq!(contact_sheet(&tiles, 9), contact_sheet(&tiles, 3));
        assert_eq!(clamp_columns(3, 0), 1);
        assert_eq!(clamp_columns(3, 9), 3);
    }

    #[test]
    fn a_sheet_of_no_tiles_has_no_pixels_and_is_not_written() {
        let (width, height, rgb) = contact_sheet(&[], 4);
        assert_eq!((width, height), (0, 0));
        assert!(rgb.is_empty());

        let path = std::env::temp_dir().join("xtask-sheet-empty.png");
        let error = write_png(&path, width, height, &rgb).unwrap_err();
        assert!(error.to_string().contains("0x0"), "{error}");
        assert!(!path.exists(), "a 0x0 PNG must not be written");
    }

    #[test]
    fn columns_for_gives_the_squarest_grid() {
        assert_eq!(columns_for(1), 1);
        assert_eq!(columns_for(2), 2);
        assert_eq!(columns_for(4), 2);
        assert_eq!(columns_for(5), 3);
        assert_eq!(columns_for(21), 5);
        assert_eq!(columns_for(42), 7);
    }

    #[test]
    fn a_part_name_becomes_a_file_name_that_cannot_leave_its_directory() {
        // The parts are named the demo names them, `&` and all; a separator in
        // one must not be able to point the sheet at another directory.
        assert_eq!(file_stem("LNS&ZOOM"), "LNS&ZOOM");
        assert_eq!(file_stem("U2E"), "U2E");
        assert_eq!(file_stem("a/b"), "a_b");
        assert_eq!(file_stem("../etc/passwd"), ".._etc_passwd");
        assert_eq!(file_stem(""), "sheet");
    }

    #[test]
    fn a_written_sheet_reads_back_as_its_own_pixels() {
        let tiles = [tile(3, 2, 0x5a, "one"), tile(3, 2, 0x6b, "two")];
        let (width, height, rgb) = contact_sheet(&tiles, 2);
        let path = std::env::temp_dir().join(format!("xtask-sheet-{}.png", std::process::id()));

        write_png(&path, width, height, &rgb).unwrap();

        let decoder = png::Decoder::new(fs::File::open(&path).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!((info.width, info.height), (width, height));
        assert_eq!(&buffer[..info.buffer_size()], &rgb[..]);
        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn a_tile_whose_buffer_is_short_does_not_take_the_sheet_with_it() {
        // A sheet is a debug picture. A tile built with the wrong length draws
        // what it has; it does not panic and it does not blank the others.
        let broken = Tile {
            width: 4,
            height: 4,
            rgb: vec![0xEE; 4 * 2 * 3],
            label: "short".into(),
        };
        let tiles = [broken, tile(4, 4, 0x22, "whole")];
        let (width, _height, rgb) = contact_sheet(&tiles, 2);

        assert_eq!(pixel(&rgb, width, 3, 3), [0xEE; 3]);
        assert_eq!(pixel(&rgb, width, 3, 8), GUTTER_COLOUR);
        assert_eq!(pixel(&rgb, width, 11, 3), [0x22; 3]);
    }
}
