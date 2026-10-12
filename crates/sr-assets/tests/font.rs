//! The font strips — `ALKU/FONA.UH`, `ENDSCRL/FONA.UH`, `ALKU/FONA.LBM`,
//! `ALKU/FONA2.LBM`, `ENDSCRL/FONA.LBM` — pinned against the parts' own text
//! routines.
//!
//! Every file these tests read is a curated copy under `assets/`, verified
//! byte-for-byte against `upstream/SecondReality/` at commit `071a82e` by task
//! A1. The geometry is not off the format (they are an Origin PIC and an IFF
//! `PBM `: two decoders, four files) but off the code that draws with them:
//!
//! - **The rows.** `ENDSCRL/MAIN.C:6` is `#define FONAY 30` and its scan reads
//!   that many rows of the strip for every character (`:107`, `:114`); the
//!   strip is 1,500×30. `CREDITS/MAIN.C:29` is the same with 32, and
//!   `ALKU/MAIN.C:218`/`:225` scan 32 rows of a strip that has 30.
//! - **The cell.** Both strips' routines put their blank's pixels in the
//!   strip's *last* 20 columns — `fonap[32]=1500-20; fonaw[32]=16;`
//!   (`ENDSCRL/MAIN.C:123-124`, `ALKU/MAIN.C:234-235`) — so the cell is 20
//!   columns wide and the strip is a whole number of them: 1,500/20 = 75, and
//!   the routines' character tables hold 74 characters (`ENDSCRL/MAIN.C:13`,
//!   `ALKU/MAIN.C:40`) with the blank as the 75th. `CREDITS/MAIN.C:270-271`
//!   says 32 and 8 instead, and 1,500/32 is 46.875.
//! - **The characters.** Each `fonaorder` opens `ABCDEFGHIJKLMNOPQRSTUVWX`.
//!
//! **The cells are not where the glyphs are.** Both routines *scan* for the
//! glyphs — a glyph is a run of columns with ink in it, and the run's width is
//! the glyph's width (`ENDSCRL/MAIN.C:103-122`) — and the runs are
//! proportional: the 74 of `ENDSCRL/FONA.UH` are 3..25 columns wide and 41 of
//! them straddle one of the 20-column cells
//! (`the_glyphs_do_not_sit_on_the_cells_they_are_cut_into`). So the tests
//! below check the glyph cells against a test-local scan of the decoded image
//! — a wrong *pitch* would move every one of them — and check the cell
//! separately, against the two lines of C that state it.

use std::path::Path;

use rstest::rstest;
use sr_assets::font::Font;
use sr_assets::{load_path, raw_surface, AssetError};

/// `ENDSCRL`'s cell: `FONAY` rows (`ENDSCRL/MAIN.C:6`) of the 20 columns its
/// blank cell states (`:123-124`).
const CELL: (u16, u16) = (20, 30);

/// The strip as the parts see it — the decoded image — and the font the
/// module builds over it. `first` is the first cell's character: each
/// `fonaorder` opens `A` (`ENDSCRL/MAIN.C:13`).
fn font_of(path: &str) -> Font {
    let image = load_path(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
    Font::from_image(image, CELL, 'A').unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A cell's pixels as the image holds them: `width` columns of the rows at
/// `y`, from `x`. Deliberately a test-local read of the surface, not the
/// loader's: if the loader's row arithmetic were wrong, this one would have to
/// be wrong the same way to hide it.
fn rect(image: &sr_assets::Image, x: u16, width: u16, rows: u16) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..rows as usize {
        let row = image.surface.row(y);
        out.extend_from_slice(&row[x as usize..x as usize + width as usize]);
    }
    out
}

/// The glyph cells the parts' routines find, run here over the decoded image:
/// maximal runs of columns holding ink, left to right. A test-local scan for
/// the same reason as `rect`.
fn scanned_cells(image: &sr_assets::Image) -> Vec<(u16, u16)> {
    let rows = image.height() as usize;
    let inked = |x: usize| (0..rows).any(|y| image.surface.get(x, y).unwrap() != 0);
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

/// Every cell's width as the font hands it out: `glyph().len() / rows`. The
/// *starts* cannot be recovered by accumulating them — the strip's cells are
/// separated by 1..3 blank columns, and `ALKU`'s last two by four — so the
/// starts come from `scanned_cells`, which reads them off the image.
fn widths_of(font: &Font, rows: u16) -> Vec<u16> {
    let mut out = Vec::new();
    let mut i = 0u32;
    while let Some(glyph) = font.glyph(char::from_u32('A' as u32 + i).unwrap()) {
        out.push((glyph.len() / rows as usize) as u16);
        i += 1;
    }
    out
}

/// The cell starts and widths a test-local scan finds, for comparison with
/// what the font hands out.
fn scanned_boxes(image: &sr_assets::Image) -> (Vec<u16>, Vec<u16>) {
    let cells = scanned_cells(image);
    (
        cells.iter().map(|&(x, _)| x).collect(),
        cells.iter().map(|&(_, w)| w).collect(),
    )
}

/// The strip's cells are the runs its own routine scans for, and every glyph
/// is the image's bytes at its own cell — checked pixel for pixel against a
/// rectangle this test reads out of the surface itself. The cells named are
/// the interesting ends of the measurement: cell 0 is `A` (20 columns), cell 2
/// is `C`, which starts at column 37 — *inside* the 20-column cell that holds
/// `B` — cell 22 is `W`, the widest at 25 columns, and cell 60 is `!`, the
/// narrowest at 3.
///
/// A font sliced at any pitch instead of scanned would fail this: its cells
/// would start at 0, 20, 40, … and cell 2 would be 20 columns wide.
///
/// **`glyph` addresses cells, not characters**, because the part's order is
/// not ASCII-contiguous: `ENDSCRL/MAIN.C:13` is
/// `ABCDEFGHIJKLMNOPQRSTUVWX` `abcdefghijklmnopqrstuvwxyz` `0123456789`
/// `!?,.:` two Swedish bytes `()+-*='` — 74 characters, and `first` is the
/// first of them. So `glyph('C')` and `glyph('W')` are the strip's 3rd and
/// 23rd cells and *do* hold `C` and `W` (the order is contiguous through
/// `X`), `glyph('!')` is `None` — `!` sorts before the first cell — and
/// `glyph('a')` is the strip's 33rd cell, which holds `i`. All three of those
/// are asserted below, so a caller that assumes ASCII has the surprise in a
/// test rather than in a rendering.
#[test]
fn the_strips_cells_are_the_runs_its_own_routine_scans_for() {
    let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
    let font = Font::from_image(image, CELL, 'A').unwrap();

    let cell_at = |i: u32| char::from_u32('A' as u32 + i).unwrap();
    assert_eq!(font.cell(), CELL);
    assert_eq!(font.glyph('A').unwrap().len(), 20 * 30);
    assert_eq!(font.glyph('C').unwrap().len(), 14 * 30);
    assert_eq!(font.glyph('W').unwrap().len(), 25 * 30);
    assert_eq!(
        font.glyph(cell_at(60)).unwrap().len(),
        3 * 30,
        "cell 60, the strip's narrowest, is the order's '!'"
    );
    assert_eq!(font.glyph('!'), None, "'!' is before the first cell");
    assert_eq!(
        font.glyph('a').unwrap().len(),
        font.glyph(cell_at(32)).unwrap().len(),
        "'a' does not reach the order's 'a' (cell 24); it reaches cell 32, 'i'"
    );
    assert_eq!(font.glyph(cell_at(74)), None, "the strip has 74 cells");
    assert_eq!(font.glyph('\u{10ffff}'), None);

    let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
    let (starts, widths) = scanned_boxes(&image);
    assert_eq!(
        widths_of(&font, CELL.1),
        widths,
        "the widths the font hands out must be the runs a scan of the image finds"
    );
    assert_eq!(
        starts[2], 37,
        "and the starts are the scan's, not a pitch's"
    );
    for (i, (x, width)) in scanned_cells(&image).into_iter().enumerate() {
        let ch = char::from_u32('A' as u32 + i as u32).unwrap();
        assert_eq!(
            font.glyph(ch).unwrap(),
            rect(&image, x, width, CELL.1),
            "cell {i} ({ch:?}) is not the image's own rectangle at column {x}"
        );
    }
}

/// The measurement the module's `Unsupported` arm defends: the glyphs are
/// **not** on the cells they are stored under. 41 of the strip's 74 cells
/// straddle a 20-column boundary, and cell 2 — `C` — starts at column 37,
/// inside the second cell rather than at the start of the third. A font built
/// by cutting the strip into 20-column cells would put `B`'s neighbour's ink
/// in `B`'s cell and every later glyph one cell off, and
/// `the_strips_cells_are_the_runs_its_own_routine_scans_for` only passes
/// because the cells come from the scan.
#[test]
fn the_glyphs_do_not_sit_on_the_cells_they_are_cut_into() {
    let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
    let cells = scanned_cells(&image);
    assert_eq!(cells.len(), 74, "one cell per character of fonaorder");

    let straddling = cells
        .iter()
        .filter(|&&(x, width)| x / CELL.0 != (x + width - 1) / CELL.0)
        .count();
    assert_eq!(
        straddling, 41,
        "41 of the 74 cells cross a cell boundary — the strip is proportional"
    );

    let (c_start, c_width) = cells[2];
    assert_eq!((c_start, c_width), (37, 14), "cell 2 is 'C'");
    assert_ne!(
        c_start / CELL.0,
        2,
        "'C' does not start in the third 20-column cell, or this test would be pointless"
    );
}

/// The other half of the geometry, and the arithmetic the strip was laid out
/// with: the routine's blank is the strip's **last** cell
/// (`fonap[32]=1500-20`, `ENDSCRL/MAIN.C:123`), which is 74 cells along —
/// `74 * 20 == 1480 == 1500 - 20` — and the 74 characters of `fonaorder`
/// (`:13`) plus that blank are the strip's 75 cells exactly, `1500 / 20`.
/// The blank's columns hold no ink, which is why the scan cannot find it and
/// the routine has to place it by hand (`:124` gives it a width of 16).
#[test]
fn the_blank_cell_the_routine_places_is_the_strips_last_cell() {
    let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
    assert_eq!(1500 / CELL.0, 75, "the strip is 75 cells of 20 columns");
    assert_eq!(
        74 * CELL.0,
        1500 - 20,
        "the blank is the 75th, and last, cell"
    );
    assert_eq!(fonaorder_len("ENDSCRL"), 74, "ENDSCRL/MAIN.C:13");

    for y in 0..CELL.1 {
        for x in 1500 - CELL.0..1500 {
            assert_eq!(
                image.surface.get(x as usize, y as usize),
                Some(0),
                "the blank cell at column {x} row {y} holds ink"
            );
        }
    }
    // And the strip's ink stops well before it — the blank is at the end of a
    // strip whose glyphs are packed into its first 1,009 columns.
    let cells = scanned_cells(&image);
    assert_eq!(cells.last(), Some(&(1006, 3)), "the last glyph is \"'\"");
}

/// The two numbers in each `fonaorder`, counted out of the source: the tests
/// cite the part's own character table, so the reading of it is checked here
/// rather than trusted. `ALKU`'s has two more entries than `ENDSCRL`'s — the
/// two 215-column pictures it prints as characters (`ALKU/MAIN.C:75-76`).
fn fonaorder_len(part: &str) -> usize {
    let path = format!("../../upstream/SecondReality/{part}/MAIN.C");
    let source = std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let at = source
        .windows(10)
        .position(|w| w == b"fonaorder=")
        .unwrap_or_else(|| panic!("{path}: no fonaorder"));
    let rest = &source[at + 10 + 1..];
    let end = rest.iter().position(|&b| b == b'"').expect("closing quote");
    rest[..end].iter().filter(|&&b| b != b'\\').count()
}

/// A cell that does not divide the strip is refused, not truncated: the last
/// cell would be a glyph cut in half, and a font built from it would lose a
/// character silently. So are the numbers that belong to *other* parts'
/// routines — `ALKU`'s 32-row read applied to this 30-row strip, and
/// `CREDITS`' 32-column cell, which does not divide 1,500 either — because a
/// cell is read off the part that draws with the strip, never borrowed.
///
/// Both are `Unsupported` rather than `Truncated`: nothing here is a short
/// read of a file (the image is decoded and whole), it is a geometry no
/// shipped part uses, and guessing a meaning for it would invent behaviour.
#[rstest]
#[case((20, 31), "31", "a cell one row taller than the strip")]
#[case((20, 32), "32", "ALKU/MAIN.C:218 reads 32 rows of a 30-row strip")]
#[case((17, 30), "17", "1,500 is not a whole number of 17-column cells")]
#[case((16, 30), "16", "1,500 is not a whole number of 16-column cells either")]
#[case((32, 32), "32", "CREDITS/MAIN.C:270-271's cell, on a strip it does not divide")]
#[case((29, 30), "29", "1,500/29 = 51.7")]
fn a_cell_that_does_not_divide_the_strip_is_refused(
    #[case] cell: (u16, u16),
    #[case] named: &str,
    #[case] why: &str,
) {
    let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
    let err = Font::from_image(image, cell, 'A').unwrap_err();
    let AssetError::Unsupported { what, .. } = &err else {
        panic!("{why}: expected Unsupported, got {err}");
    };
    assert!(
        what.contains(named),
        "{why}: the message must name {named}: {what}"
    );
}

/// A width that *divides* the strip but is not the routine's is accepted, and
/// the glyphs do not change: they come from the scan, never from the cell.
///
/// `ENDSCRL`'s routine states 20 columns (`ENDSCRL/MAIN.C:123-124`), but 1,500
/// also divides by 25 — the widest glyph, `W` — and by 30 and 50
/// (`src/font.rs`'s module docs count all three). Nothing in the strip says
/// which of them is *the* cell, so the loader may not invent a refusal for the
/// ones the routine did not state; it refuses only a width the strip cannot be
/// cut into. The contrast with the test above is the point, and so is the
/// equality: a caller that states a different cell gets the same pixels back,
/// and only [`Font::cell`] answers with what the caller stated.
#[test]
fn a_dividing_width_that_is_not_the_routine_s_is_accepted() {
    let pick = |cell: (u16, u16)| {
        let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
        Font::from_image(image, cell, 'A').unwrap_or_else(|e| panic!("cell {cell:?}: {e}"))
    };
    let routine = pick(CELL);
    let wide = pick((25, 30));

    assert_eq!(
        wide.cell(),
        (25, 30),
        "cell() reports the geometry the caller stated"
    );
    for ch in ['A', 'B', 'W', 'X'] {
        assert_eq!(
            wide.glyph(ch),
            routine.glyph(ch),
            "{ch}: the glyph is the scan's run, not the cell's slice"
        );
    }
}

/// A cell with no pixels in it is refused before anything is read out of the
/// image — it is not a strip one line of text tall, so there is no geometry to
/// slice. A cell whose rows are not the strip's is refused too, in the cases
/// above: the routine reads every row of its strip for every character, so
/// there is no such thing as a strip taller than its cell.
#[test]
fn a_cell_with_no_pixels_is_refused() {
    for cell in [(0, 30), (20, 0), (0, 0)] {
        let image = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();
        assert!(
            matches!(
                Font::from_image(image, cell, 'A'),
                Err(AssetError::Unsupported { .. })
            ),
            "a {cell:?} cell"
        );
    }
}

/// `ALKU`'s strip is the same font, and `ALKU/MAIN.C` reads 32 rows of it
/// (`:218`, `:274`) — two rows more than the file has, because the `fona.inc`
/// fragment is followed by `dw 1500*5 dup(?)`
/// (`ALKU/INCLUDE.ASM:19`), 3,000 bytes of padding the linker left with
/// whatever it left. The port reads the strip: 30 rows, cell `(20, 30)`, and
/// the two parts' *cells* agree while their row counts do not — which is the
/// cell that gets refused above, and not this one.
///
/// `ALKU`'s character table has two entries more than `ENDSCRL`'s
/// (`:40` against `ENDSCRL/MAIN.C:13`), and the strip has two cells more:
/// 215-column pictures at columns 1014 and 1231, which the part prints as the
/// characters `\x8f` and `\x99` (`ALKU/MAIN.C:75-76`).
#[test]
fn the_two_parts_cells_agree_and_their_rows_do_not() {
    let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    let font = Font::from_image(image, CELL, 'A').unwrap();
    assert_eq!(fonaorder_len("ALKU"), 76);

    let (starts, widths) =
        scanned_boxes(&load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap());
    assert_eq!(starts.len(), 76);
    assert_eq!(widths_of(&font, CELL.1), widths);

    let (end_starts, _) =
        scanned_boxes(&load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap());
    assert_eq!(
        starts[..74],
        end_starts[..],
        "the two strips hold the same 74 cells"
    );
    assert_eq!(starts[74..], [1014, 1231]);
    assert_eq!(widths[74..], [215, 215]);
    assert_eq!(font.glyph('A').unwrap().len(), 20 * 30);

    // And the row counts *do* disagree: `ALKU/MAIN.C:218`'s 32 is refused
    // while `ENDSCRL/MAIN.C:6`'s 30 is not.
    let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    assert!(matches!(
        Font::from_image(image, (20, 32), 'A'),
        Err(AssetError::Unsupported { .. })
    ));
}

/// The two `.UH` strips are **not the same picture**: 5,063 of their pixels
/// differ, and every one of them is at a column at or past 1,014 — the
/// right-hand art `ALKU` carries and `ENDSCRL` leaves blank — while 502 of
/// their 768 palette bytes differ. Both halves matter. The glyph cells
/// themselves are byte-identical, which is what makes the two files a
/// *cross-check* of the cell geometry rather than two measurements of one
/// picture: two independently converted strips, two palettes, one layout, and
/// a scan of each finds the same 74 cells.
#[test]
fn the_two_strips_are_one_font_and_not_one_picture() {
    let alku = load_path(Path::new("../../assets/ALKU/FONA.UH")).unwrap();
    let endscrl = load_path(Path::new("../../assets/ENDSCRL/FONA.UH")).unwrap();

    let mut differing = 0;
    let mut leftmost = u16::MAX;
    for y in 0..CELL.1 as usize {
        for x in 0..1500usize {
            if alku.surface.get(x, y) != endscrl.surface.get(x, y) {
                differing += 1;
                leftmost = leftmost.min(x as u16);
            }
        }
    }
    assert_eq!(differing, 5_063, "the strips' pixels");
    assert_eq!(
        leftmost, 1014,
        "every difference is in the art, not the glyphs"
    );

    let palette_differences = (0..256)
        .filter(|&i| alku.palette.entry(i as u8) != endscrl.palette.entry(i as u8))
        .count();
    assert_eq!(palette_differences, 252, "the strips' palettes");

    // The cells and their pixels are the same in both files.
    let alku_font = Font::from_image(alku, CELL, 'A').unwrap();
    let endscrl_font = Font::from_image(endscrl, CELL, 'A').unwrap();
    for i in 0..74u32 {
        let ch = char::from_u32('A' as u32 + i).unwrap();
        assert_eq!(
            alku_font.glyph(ch).unwrap(),
            endscrl_font.glyph(ch).unwrap(),
            "cell {i} ({ch:?}) differs between the two strips"
        );
    }
}

/// The strips and their source art, through two decoders: `FONA.UH` is the
/// Origin PIC `GRAB/LBM2U.EXE` wrote from `FONA.LBM` (the toolchain's own
/// conversion, `GRAB/LBM2U.C`), so the glyphs must come out of A2's decoder
/// and A3's IFF decoder byte for byte the same, and `ENDSCRL`'s pair too.
/// `ALKU/FONA2.LBM` is the same font a second time — its first 74 cells are
/// `FONA.UH`'s, byte for byte — and differs only in its own two pictures at
/// the end, which are 185 and 215 columns wide against `FONA.UH`'s 215 and
/// 215.
#[rstest]
#[case("../../assets/ALKU/FONA.LBM", 40_860)]
#[case("../../assets/ENDSCRL/FONA.LBM", 27_960)]
fn the_source_art_and_the_strip_are_the_same_glyphs(#[case] art: &str, #[case] compared: usize) {
    let part = if art.contains("ALKU") {
        "ALKU"
    } else {
        "ENDSCRL"
    };
    let strip = font_of(&format!("../../assets/{part}/FONA.UH"));
    let art = font_of(art);

    let mut seen = 0;
    for i in 0..74u32 {
        let ch = char::from_u32('A' as u32 + i).unwrap();
        let (a, b) = (strip.glyph(ch).unwrap(), art.glyph(ch).unwrap());
        assert_eq!(a, b, "{part}: cell {i} ({ch:?})");
        seen += a.len();
    }
    if part == "ALKU" {
        for i in [74u32, 75] {
            let ch = char::from_u32('A' as u32 + i).unwrap();
            assert_eq!(strip.glyph(ch).unwrap(), art.glyph(ch).unwrap());
            seen += strip.glyph(ch).unwrap().len();
        }
    }
    assert_eq!(seen, compared, "the bytes this comparison covered");
}

/// `ALKU/FONA2.LBM`, the tree's second font strip: the same 76 cells as
/// `ALKU/FONA.UH` in its first 74, and two pictures of its own at the end.
#[test]
fn the_second_strip_differs_only_in_its_own_two_pictures() {
    let first = font_of("../../assets/ALKU/FONA.UH");
    let second = font_of("../../assets/ALKU/FONA2.LBM");

    let (starts, widths) =
        scanned_boxes(&load_path(Path::new("../../assets/ALKU/FONA2.LBM")).unwrap());
    assert_eq!(starts.len(), 76);
    assert_eq!(widths_of(&second, CELL.1), widths);
    for i in 0..74u32 {
        let ch = char::from_u32('A' as u32 + i).unwrap();
        assert_eq!(
            second.glyph(ch).unwrap(),
            first.glyph(ch).unwrap(),
            "cell {i} ({ch:?})"
        );
    }
    assert_eq!(starts[74..], [1011, 1231]);
    assert_eq!(widths[74..], [215, 185]);
    assert_ne!(
        second.glyph(char::from_u32('A' as u32 + 75).unwrap()),
        first.glyph(char::from_u32('A' as u32 + 75).unwrap())
    );
}

/// The strips are 2-bit masks — every value in every glyph is 0..3 — and the
/// loader hands them back as the file holds them. The parts fold those values
/// themselves (`ALKU/MAIN.C:173-181` onto the top two bits of a screen byte,
/// `ENDSCRL/MAIN.C:80-87` into four EGA planes), so a loader that "helpfully"
/// widened them would change what every part draws.
#[test]
fn a_glyph_is_the_strips_own_values() {
    for path in [
        "../../assets/ALKU/FONA.UH",
        "../../assets/ENDSCRL/FONA.UH",
        "../../assets/ALKU/FONA2.LBM",
    ] {
        let font = font_of(path);
        let mut values = [0usize; 256];
        for i in 0..76u32 {
            let Some(glyph) = font.glyph(char::from_u32('A' as u32 + i).unwrap()) else {
                break;
            };
            for &v in glyph {
                values[v as usize] += 1;
            }
        }
        assert!(
            values[4..].iter().all(|&n| n == 0),
            "{path}: a glyph value above 3: {:?}",
            values[4..].iter().position(|&n| n != 0)
        );
        assert!(
            values[1..4].iter().all(|&n| n > 0),
            "{path}: the strip's other three values are in its glyphs too"
        );
    }
}

/// `GRAB/BLOWFNT.1BM` is **not** a font strip this loader can read, and it is
/// not loaded as one. It is 64,000 bytes of bare 320×200 pixels with no
/// header and no magic, so `raw_surface` is its door
/// (`sr_assets::sniff` cannot see it, and its size identifies nothing: three
/// 64,000-byte files in this tree are three different shapes). Nothing links
/// it and nothing opens it: no `.OBJ` or `.EXE` in either tree holds any of
/// its glyph rows, and no part opens a `.1bm` — the extension is named once in
/// all of `upstream/`, in a tool's comment (`GRAB/GFXSAVE.H:5`).
///
/// Its pixels do hold a grid of cells, and the measurement is worth keeping
/// because it is what makes the temptation: vertical rules in every one of the
/// canvas's first 160 rows at columns 31 + 32k, horizontal rules filling rows
/// 31 + 32k, glyphs (`A`, `B`, `C`, `D`, …) in the 50 cells between them — and
/// then nothing at all below row 159. Forty of the canvas's 200 rows are
/// outside the grid, so even the grid's own cell does not tile its canvas, and
/// no part's text routine states what the cell is. A font built from it here
/// would be this task's invented cell grid; there is none. The rules are
/// measured below so the claim is one a test makes rather than one a comment
/// asserts.
#[test]
fn blowfnt_is_not_read_as_a_font_strip() {
    let bytes = std::fs::read("../../assets/GRAB/BLOWFNT.1BM").unwrap();
    assert_eq!(bytes.len(), 64_000);
    let image = raw_surface(&bytes, 320, 200).unwrap();

    // Ten vertical rules, each inked in exactly the grid's 160 rows…
    for k in 0..10usize {
        let x = 31 + 32 * k;
        let inked = (0..200)
            .filter(|&y| image.surface.get(x, y) != Some(0))
            .count();
        assert_eq!(inked, 160, "the rule column at x = {x}");
    }
    // …five horizontal rules, each filling the canvas, the last at row 159…
    for k in 0..5usize {
        let y = 31 + 32 * k;
        let inked = (0..320)
            .filter(|&x| image.surface.get(x, y) != Some(0))
            .count();
        assert_eq!(inked, 320, "the rule row at y = {y}");
    }
    // …and 40 rows of canvas below the grid that hold nothing at all.
    for y in 160..200 {
        assert_eq!(
            image.surface.row(y).iter().filter(|&&v| v != 0).count(),
            0,
            "row {y} is outside the grid and must be empty"
        );
    }

    // And the loader refuses it: 320×200 is 6¼ cells of 32 rows, and no part
    // says what its cell is.
    let err = Font::from_image(image, (32, 32), 'A').unwrap_err();
    let AssetError::Unsupported { .. } = err else {
        panic!("expected Unsupported, got {err}");
    };
}
