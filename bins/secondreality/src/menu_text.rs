//! The text layer the port's own menus share: the vault menu and the settings screen.
//!
//! Both screens are new surfaces and **not** fidelity ones — there is no original frame
//! to match — but they draw their text with the demo's own font, `ALKU/FONA.UH`, read by
//! the same [`Font`] the parts use, at 1:1. This module is that drawing: a character
//! table, proportional layout, the `>` marker and the four-group palette. The screens own
//! their layouts and their state; this owns the glyphs.
//!
//! The strip's cells are proportional runs of ink, 3..25 columns wide
//! (`sr_assets::font`'s module docs): a glyph advances by its own width here,
//! a space by ten columns, and the 2-bit strip values 1..3 are the ink levels
//! of whichever palette group the text is drawn in (0 is the black
//! background). Each group is three consecutive palette entries, faint to
//! full, so the strip's own antialiasing survives.

use sr_assets::font::Font;
use sr_engine::{IndexedFrame, Palette};

/// `fonaw[32]=16` in the source's routines; the menu's own layout uses ten
/// columns for a space.
const SPACE_ADVANCE: i32 = 10;

/// The gap between two glyphs. The strip's scanned runs exclude the blank
/// columns between them, so a little tracking keeps the words legible when
/// they are set side by side.
const GLYPH_GAP: i32 = 2;

/// The strip's own character order, as `ALKU/MAIN.C:40` states it: `A`..`X`, then
/// `a`..`z`, the digits and `!?,.:`, two cells the source writes as the byte `0x8F`
/// (cells 65 and 66), the seven marks `( ) + - * = '` (cells 67..73), and two last
/// cells (`0x8F`, `0x99`) that the table stops short of.
///
/// The order is **not** ASCII-contiguous past `X`: [`Font::glyph`] indexes the
/// scanned cells by `ch - 'A'`, so this table is what turns a character into
/// the placeholder character whose index is the cell's place in the order.
/// The four cells the source prints as pictures (`ALKU/MAIN.C:75-76`; two 215-column
/// ones are the strip's last) are never drawn: they are not printable ASCII, so the
/// table holds `0x8F` for the two before the marks, which keeps every mark's place and
/// which no text the menus draw contains.
const FONA_ORDER: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\u{8f}\u{8f}()+-*='";

/// How far the strip's lowercase `y` sits below an uppercase glyph's top. The
/// order has no uppercase `Y` (it stops at `X`), so the title's `Y` is the
/// strip's own `y` cell raised onto the cap line.
const Y_DROP: i32 = 4;

// The palette groups: entry `base + value - 1` is ink level `value` (1..=3) of
// that role, so each role owns three consecutive entries.
pub const TITLE_INK: u8 = 1;
pub const ENTRY_INK: u8 = 4;
pub const SELECT_INK: u8 = 7;
pub const FOOTER_INK: u8 = 10;

/// The menus' colours, all six-bit DAC values: black, white, grey, amber and a
/// dim blue, each ink group three entries wide (faint, medium, full).
pub fn menu_palette() -> Palette {
    let mut palette = Palette::new();
    let groups: [(u8, [[u8; 3]; 3]); 4] = [
        // Title: white.
        (TITLE_INK, [[16, 16, 16], [40, 40, 40], [63, 63, 63]]),
        // Entries: grey.
        (ENTRY_INK, [[8, 8, 8], [22, 22, 22], [36, 36, 36]]),
        // Selection: amber.
        (SELECT_INK, [[24, 10, 0], [44, 22, 0], [63, 40, 0]]),
        // Footer: dim blue.
        (FOOTER_INK, [[4, 6, 16], [8, 12, 28], [14, 20, 44]]),
    ];
    for (base, shades) in groups {
        for (step, rgb) in shades.into_iter().enumerate() {
            palette.set_entry(base + step as u8, rgb);
        }
    }
    palette
}

/// The cell character whose index in the strip is `ch`'s place in
/// [`FONA_ORDER`] — the placeholder [`Font::glyph`] expects.
///
/// The strip has no uppercase `Y` (the order runs `A`..`X`), so a `Y` falls
/// back to the lowercase `y` cell, the font's only Y-shaped glyph; the caller
/// raises it onto the cap line.
fn cell_of(ch: char) -> Option<char> {
    let position = |wanted: char| FONA_ORDER.chars().position(|c| c == wanted);
    let at = position(ch).or_else(|| position(ch.to_ascii_lowercase()))?;
    char::from_u32('A' as u32 + at as u32)
}

/// The glyph for `ch`, or `None` for one the strip has no cell for.
fn glyph_of(font: &Font, ch: char) -> Option<&[u8]> {
    font.glyph(cell_of(ch)?)
}

/// How many columns `text` occupies when drawn.
pub fn text_width(font: &Font, text: &str) -> i32 {
    let mut width = 0;
    let mut glyphs: i32 = 0;
    for ch in text.chars() {
        if let Some(glyph) = glyph_of(font, ch) {
            width += (glyph.len() / font.cell().1 as usize) as i32;
            glyphs += 1;
        } else {
            // A space — the strip's blank is not a cell the scan finds — and
            // anything else the menu does not draw.
            width += SPACE_ADVANCE;
        }
    }
    // The gap follows a glyph, never the last one.
    width + (glyphs.saturating_sub(1) * GLYPH_GAP)
}

/// Draw `text` with its first glyph's cell at `(x, y)`, in `base`'s ink group.
pub fn draw_text(frame: &mut IndexedFrame, font: &Font, x: i32, y: i32, text: &str, base: u8) {
    let rows = font.cell().1 as usize;
    let mut pen = x;
    for ch in text.chars() {
        let Some(glyph) = glyph_of(font, ch) else {
            pen += SPACE_ADVANCE;
            continue;
        };
        let width = glyph.len() / rows;
        let shift = if ch == 'Y' { -Y_DROP } else { 0 };
        for (row, line) in glyph.chunks_exact(width).enumerate() {
            for (column, &value) in line.iter().enumerate() {
                if value == 0 {
                    continue;
                }
                let pixel = base + value - 1;
                set(frame, pen + column as i32, y + row as i32 + shift, pixel);
            }
        }
        pen += width as i32 + GLYPH_GAP;
    }
}

/// Draw `text` centred horizontally, its ink starting at `y`.
pub fn draw_centered(frame: &mut IndexedFrame, font: &Font, y: i32, text: &str, base: u8) {
    let x = (frame.width as i32 - text_width(font, text)) / 2;
    draw_text(frame, font, x, y, text, base);
}

/// A row's `>`: a two-pixel-wide chevron, eight rows tall, in the selection's full ink.
pub fn draw_marker(frame: &mut IndexedFrame, x: i32, y: i32) {
    for row in 0..8 {
        let arm = if row <= 3 { row } else { 7 - row };
        for thickness in 0..2 {
            set(frame, x + arm + thickness, y + row, SELECT_INK + 2);
        }
    }
}

/// One pixel, clipped to the frame.
fn set(frame: &mut IndexedFrame, x: i32, y: i32, value: u8) {
    if x < 0 || y < 0 || x >= frame.width as i32 || y >= frame.height as i32 {
        return;
    }
    let at = y as usize * frame.width as usize + x as usize;
    frame.pixels[at] = value;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_assets::load_path;
    use std::path::Path;

    /// `ALKU/FONA.UH` read the way the demo's own text routine reads it (see
    /// `vault_menu`'s tests, which read it the same way).
    fn test_font() -> Font {
        let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).expect("the font strip");
        Font::from_image(image, (20, 30), 'A').expect("the strip read as ALKU reads it")
    }

    #[test]
    fn the_table_is_the_sources_order_up_to_each_cell() {
        // `ALKU/MAIN.C:40`: 24 + 26 + 10 letters and digits, `!?,.:` at 60..65, two
        // pictures at 65 and 66, then `( ) + - * = '` at 67..74. A cell's character is
        // `'A' + its index` (`Font::glyph`'s own convention).
        let cell = |ch| cell_of(ch).map(|c| c as u32 - 'A' as u32);
        assert_eq!(cell('A'), Some(0));
        assert_eq!(cell('X'), Some(23));
        assert_eq!(cell('a'), Some(24));
        assert_eq!(cell('z'), Some(49));
        assert_eq!(cell('0'), Some(50));
        assert_eq!(cell('9'), Some(59));
        assert_eq!(cell('!'), Some(60));
        assert_eq!(cell(':'), Some(64));
        for (at, mark) in "()+-*='".chars().enumerate() {
            assert_eq!(cell(mark), Some(67 + at as u32), "the cell of {mark:?}");
        }
        // The strip has 76 cells; the table reaches the last of the marks, 73.
        assert_eq!(FONA_ORDER.chars().count(), 74);
    }

    #[test]
    fn the_seven_marks_draw_ink_of_their_own() {
        let font = test_font();
        let mut seen = Vec::new();
        for mark in "()+-*='".chars() {
            let glyph = glyph_of(&font, mark).unwrap_or_else(|| panic!("{mark:?} has no cell"));
            let width = glyph.len() / font.cell().1 as usize;
            assert!((3..=25).contains(&width), "{mark:?} is {width} columns");
            assert!(glyph.iter().any(|&v| v != 0), "{mark:?} is blank");
            assert!(
                !seen.contains(&glyph),
                "{mark:?} draws the same glyph as an earlier mark"
            );
            seen.push(glyph);
        }
    }

    #[test]
    fn a_space_is_the_only_advance_without_a_glyph() {
        let font = test_font();
        let word = text_width(&font, "A");
        // Two glyphs and the gap between them, and a space that is not a glyph.
        assert_eq!(text_width(&font, "AA"), 2 * word + GLYPH_GAP);
        assert_eq!(text_width(&font, "A A"), 2 * word + GLYPH_GAP + SPACE_ADVANCE);
    }
}
