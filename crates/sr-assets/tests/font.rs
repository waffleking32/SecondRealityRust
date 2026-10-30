//! The font strips (`ALKU/FONA.UH`, `ENDSCRL/FONA.UH` and their `.LBM` art) against the parts'
//! own text routines, which scan a strip for ink runs (`ENDSCRL/MAIN.C:103-122`).

mod common;

use common::{asset, asset_path, upstream};
use rstest::rstest;
use sr_assets::font::Font;
use sr_assets::{load_path, Image};

fn image(file: &str) -> Image {
    load_path(&asset_path(file)).unwrap_or_else(|e| panic!("{file}: {e}"))
}

fn font_of(file: &str) -> Font {
    Font::from_image(image(file))
}

/// The cell for index `i`, in `Font::glyph`'s convention.
fn cell(i: u32) -> char {
    char::from_u32('A' as u32 + i).unwrap()
}

/// The ink runs, scanned here over the surface rather than by the module, as `(start, width)`.
fn scanned_runs(image: &Image) -> Vec<(u16, u16)> {
    let inked = |x: usize| (0..image.height() as usize).any(|y| image.surface.get(x, y).unwrap() != 0);
    let mut out = Vec::new();
    let mut x = 0usize;
    while x < image.width() as usize {
        if !inked(x) {
            x += 1;
            continue;
        }
        let start = x;
        while x < image.width() as usize && inked(x) {
            x += 1;
        }
        out.push((start as u16, (x - start) as u16));
    }
    out
}

/// The length of a part's `fonaorder`, counted out of its source.
fn fonaorder_len(part: &str) -> usize {
    let source = upstream(&format!("{part}/MAIN.C"));
    let at = source.windows(10).position(|w| w == b"fonaorder=").expect("fonaorder");
    let rest = &source[at + 11..];
    let end = rest.iter().position(|&b| b == b'"').expect("closing quote");
    rest[..end].iter().filter(|&&b| b != b'\\').count()
}

/// Every glyph is the image's own rectangle at its run, all 30 rows, and the cells are in
/// `fonaorder`'s order (74 for `ENDSCRL/MAIN.C:13`). `C` starts at column 37, inside the
/// 20-column cell that `B` would occupy on a grid; 41 of the 74 runs straddle a 20-column
/// boundary, so no uniform cut finds them. `glyph` takes a cell index: `'!'` sorts before the
/// first cell, and `'a'` reaches cell 32, the order's `i`.
#[test]
fn the_strips_cells_are_the_runs_its_own_routine_scans_for() {
    let img = image("ENDSCRL/FONA.UH");
    let runs = scanned_runs(&img);
    assert_eq!(runs.len(), fonaorder_len("ENDSCRL"));
    assert_eq!(runs.len(), 74);
    assert_eq!(runs[2], (37, 14), "C");
    assert_eq!(runs.iter().filter(|&&(x, w)| x / 20 != (x + w - 1) / 20).count(), 41);

    let font = Font::from_image(image("ENDSCRL/FONA.UH"));
    assert_eq!(font.rows(), 30);
    for (i, &(x, width)) in runs.iter().enumerate() {
        let mut want = Vec::new();
        for y in 0..30 {
            want.extend_from_slice(&img.surface.row(y)[x as usize..(x + width) as usize]);
        }
        assert_eq!(font.glyph(cell(i as u32)).unwrap(), want, "cell {i}");
    }
    assert_eq!(font.glyph('!'), None);
    assert_eq!(font.glyph('a'), font.glyph(cell(32)));
    assert_eq!(font.glyph(cell(74)), None);
    assert_eq!(font.glyph('\u{10ffff}'), None);
}

/// The routine's blank is the strip's last 20 columns (`fonap[32]=1500-20`,
/// `ENDSCRL/MAIN.C:123`), and they hold no ink: the glyphs end at column 1,009.
#[test]
fn the_blank_cell_the_routine_places_holds_no_ink() {
    let img = image("ENDSCRL/FONA.UH");
    for y in 0..30 {
        assert!(img.surface.row(y)[1480..].iter().all(|&v| v == 0), "row {y}");
    }
    assert_eq!(scanned_runs(&img).last(), Some(&(1006, 3)), "the last glyph is \"'\"");
}

/// `ALKU`'s strip holds the same 74 glyphs and two 215-column pictures after them, the
/// characters `\x8f` and `\x99` (`ALKU/MAIN.C:75-76`; its order has 76 entries, `:40`). The two
/// `.UH` files are not one picture: 5,063 pixels differ, all at or past column 1,014, and 252
/// palette entries.
#[test]
fn the_two_strips_are_one_font_and_not_one_picture() {
    let (alku, endscrl) = (image("ALKU/FONA.UH"), image("ENDSCRL/FONA.UH"));
    let runs = scanned_runs(&alku);
    assert_eq!(runs.len(), fonaorder_len("ALKU"));
    assert_eq!(runs[..74], scanned_runs(&endscrl)[..]);
    assert_eq!(runs[74..], [(1014, 215), (1231, 215)]);

    let mut differing = 0;
    let mut leftmost = usize::MAX;
    for y in 0..30 {
        for x in 0..1500 {
            if alku.surface.get(x, y) != endscrl.surface.get(x, y) {
                differing += 1;
                leftmost = leftmost.min(x);
            }
        }
    }
    assert_eq!((differing, leftmost), (5_063, 1014));
    let palettes = (0..=255u8).filter(|&i| alku.palette.entry(i) != endscrl.palette.entry(i)).count();
    assert_eq!(palettes, 252);
}

/// Each `.UH` is `GRAB/LBM2U.EXE`'s conversion of its `.LBM`, so the glyphs out of the Origin and
/// the IFF decoder are the same bytes. `ALKU/FONA2.LBM` has the same 74 glyphs and two pictures
/// of its own (215 and 185 columns).
#[rstest]
#[case("ALKU/FONA.UH", "ALKU/FONA.LBM", 76)]
#[case("ENDSCRL/FONA.UH", "ENDSCRL/FONA.LBM", 74)]
#[case("ALKU/FONA.UH", "ALKU/FONA2.LBM", 74)]
fn the_source_art_and_the_strip_are_the_same_glyphs(#[case] strip: &str, #[case] art: &str, #[case] same: u32) {
    let (strip, art) = (font_of(strip), font_of(art));
    for i in 0..same {
        assert_eq!(strip.glyph(cell(i)), art.glyph(cell(i)), "cell {i}");
    }
}

#[test]
fn the_second_strips_own_pictures_differ() {
    let runs = scanned_runs(&image("ALKU/FONA2.LBM"));
    assert_eq!(runs[74..], [(1011, 215), (1231, 185)]);
}

/// Every glyph value is the strip's own 2-bit mask (0..3, all of them used); the parts fold the
/// values themselves (`ALKU/MAIN.C:173-181`, `ENDSCRL/MAIN.C:80-87`).
#[rstest]
#[case("ALKU/FONA.UH")]
#[case("ENDSCRL/FONA.UH")]
#[case("ALKU/FONA2.LBM")]
fn a_glyph_is_the_strips_own_values(#[case] file: &str) {
    let font = font_of(file);
    let mut values = [0usize; 256];
    for glyph in (0..).map_while(|i| font.glyph(cell(i))) {
        for &v in glyph {
            values[v as usize] += 1;
        }
    }
    assert!(values[4..].iter().all(|&n| n == 0), "{file}: a value above 3");
    assert!(values[1..4].iter().all(|&n| n > 0), "{file}: all three ink values");
}

/// `GRAB/BLOWFNT.1BM` is no font strip: a bare 320x200 dump (`asset-inventory.md` §E.14) holding
/// a 10x5 grid of 32-pixel cells, rules at `31 + 32k`, that stops at row 159.
#[test]
fn blowfnt_is_a_grid_that_does_not_tile_its_canvas() {
    let bytes = asset("GRAB/BLOWFNT.1BM");
    let img = sr_assets::raw_surface(&bytes, 320, 200).unwrap();
    for k in 0..10 {
        let x = 31 + 32 * k;
        assert_eq!((0..200).filter(|&y| img.surface.get(x, y) != Some(0)).count(), 160, "x = {x}");
    }
    for k in 0..5 {
        let y = 31 + 32 * k;
        assert_eq!((0..320).filter(|&x| img.surface.get(x, y) != Some(0)).count(), 320, "y = {y}");
    }
    assert!((160..200).all(|y| img.surface.row(y).iter().all(|&v| v == 0)));
}
