//! The port's own menus: the settings screen a windowed demo run opens on (D44) and the
//! `--vault` menu (D40). Neither is a fidelity surface: there is no original frame to match.
//!
//! Both draw with the demo's font, `ALKU/FONA.UH`, at 1:1 through the same [`Font`] the
//! parts use. This module is the shared layer: the character table, proportional layout,
//! the `>` marker, the four-group palette, the key a screen acts on and the screen layout
//! both draw. Each screen ([`settings`], [`vault`]) is a pure reducer over [`Key`], so its
//! behaviour is testable without a window; the window owns the loop.
//!
//! The strip's cells are proportional runs of ink, 3..25 columns wide (`sr_assets::font`).
//! A glyph advances by its own width, a space by ten columns, and the strip values 1..3 are
//! the ink levels of the palette group the text is drawn in (0 is the background). Each
//! group is three consecutive entries, faint to full, so the strip's antialiasing survives.

pub mod settings;
pub mod vault;

use std::path::Path;

use sr_assets::font::Font;
use sr_assets::load_path;
use sr_engine::{IndexedFrame, Palette};

/// `fonaw[32]=16` in the source's routines; the menus use ten columns for a space.
const SPACE_ADVANCE: i32 = 10;

/// The gap between two glyphs: the strip's scanned runs exclude the blank columns
/// between them, so a little tracking keeps words legible.
const GLYPH_GAP: i32 = 2;

/// The strip's character order, as `ALKU/MAIN.C:40` states it: `A`..`X`, `a`..`z`, the
/// digits and `!?,.:`, two picture cells the source writes as `0x8F` (65 and 66), the seven
/// marks `( ) + - * = '` (67..73), and two last cells the table stops short of.
///
/// [`Font::glyph`] indexes cells by `ch - 'A'`, so this table turns a character into the
/// placeholder whose index is the cell's place in the order. The picture cells
/// (`ALKU/MAIN.C:75-76`) are never drawn; `0x8F` keeps every mark's place.
const FONA_ORDER: &str =
    "ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\u{8f}\u{8f}()+-*='";

/// How far the strip's lowercase `y` sits below an uppercase glyph's top. The order has
/// no uppercase `Y`, so a `Y` is the `y` cell raised onto the cap line.
const Y_DROP: i32 = 4;

/// The font strip, under `root` (the working directory for a run).
const FONT: &str = "assets/ALKU/FONA.UH";

// The palette groups: entry `base + value - 1` is ink level `value` (1..=3) of that role.
pub const TITLE_INK: u8 = 1;
pub const ENTRY_INK: u8 = 4;
pub const SELECT_INK: u8 = 7;
pub const FOOTER_INK: u8 = 10;

/// A key a menu acts on. Everything else SDL reports is [`Key::Other`], so the mapping is
/// total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Other,
}

/// The menus' font strip, read under `root` as `ALKU/MAIN.C`'s text routine reads it.
pub fn font(root: &Path) -> Result<Font, String> {
    let image = load_path(&root.join(FONT)).map_err(|error| format!("{FONT}: {error}"))?;
    Ok(Font::from_image(image))
}

/// The font as unit tests find it: they run in the binary's directory.
#[cfg(test)]
pub fn test_font() -> Font {
    font(Path::new("../..")).expect("the font strip")
}

/// The menus' colours, six-bit: black, then white, grey, amber and dim blue groups.
pub fn menu_palette() -> Palette {
    let mut palette = Palette::new();
    let groups: [(u8, [[u8; 3]; 3]); 4] = [
        (TITLE_INK, [[16, 16, 16], [40, 40, 40], [63, 63, 63]]),
        (ENTRY_INK, [[8, 8, 8], [22, 22, 22], [36, 36, 36]]),
        (SELECT_INK, [[24, 10, 0], [44, 22, 0], [63, 40, 0]]),
        (FOOTER_INK, [[4, 6, 16], [8, 12, 28], [14, 20, 44]]),
    ];
    for (base, shades) in groups {
        for (step, rgb) in shades.into_iter().enumerate() {
            palette.set_entry(base + step as u8, rgb);
        }
    }
    palette
}

/// Where a screen puts its text. Rows start at `entry_top` and step by `entry_step`.
pub struct Layout {
    pub width: u16,
    pub height: u16,
    pub subtitle: &'static str,
    pub title_top: i32,
    pub subtitle_top: i32,
    pub entry_top: i32,
    pub entry_step: i32,
    pub marker_x: i32,
    pub label_x: i32,
    pub value_x: i32,
    pub footer_top: i32,
    pub footer: &'static str,
    /// The overscan colour, if the screen has a border to show.
    pub border: Option<u8>,
}

impl Layout {
    /// The ink top of row `row`.
    pub fn row_top(&self, row: usize) -> i32 {
        self.entry_top + row as i32 * self.entry_step
    }
}

/// Draw a screen: cleared, the menu palette, the centred title and subtitle, each row's
/// label (and value, if it has one) with the `>` on `selected`, and the centred footer.
///
/// The selected row is all in the selection's ink; other rows' labels are entry ink and
/// their values title ink.
pub fn draw_screen<'r>(
    frame: &mut IndexedFrame,
    font: &Font,
    layout: &Layout,
    rows: impl IntoIterator<Item = (&'r str, Option<&'r str>)>,
    selected: usize,
) {
    debug_assert_eq!(
        (frame.width, frame.height),
        (layout.width, layout.height),
        "the {} screen is laid out for {}x{}",
        layout.subtitle,
        layout.width,
        layout.height
    );
    frame.pixels.fill(0);
    frame.palettes.reset(menu_palette());
    frame.border = layout.border;

    draw_centered(frame, font, layout.title_top, "SECOND REALITY", TITLE_INK);
    draw_centered(frame, font, layout.subtitle_top, layout.subtitle, TITLE_INK);

    for (row, (label, value)) in rows.into_iter().enumerate() {
        let y = layout.row_top(row);
        let (label_ink, value_ink) = if row == selected {
            draw_marker(frame, layout.marker_x, y + 2);
            (SELECT_INK, SELECT_INK)
        } else {
            (ENTRY_INK, TITLE_INK)
        };
        draw_text(frame, font, layout.label_x, y, label, label_ink);
        if let Some(value) = value {
            draw_text(frame, font, layout.value_x, y, value, value_ink);
        }
    }

    draw_centered(frame, font, layout.footer_top, layout.footer, FOOTER_INK);
}

/// The cell character whose index in the strip is `ch`'s place in [`FONA_ORDER`]; a `Y`
/// falls back to the lowercase `y` cell.
fn cell_of(ch: char) -> Option<char> {
    let position = |wanted: char| FONA_ORDER.chars().position(|c| c == wanted);
    let at = position(ch).or_else(|| position(ch.to_ascii_lowercase()))?;
    char::from_u32('A' as u32 + at as u32)
}

/// The glyph for `ch`, or `None` for one the strip has no cell for.
fn glyph_of(font: &Font, ch: char) -> Option<&[u8]> {
    font.glyph(cell_of(ch)?)
}

/// How many columns `text` occupies when drawn: glyph widths, a gap after every glyph but
/// the last, and [`SPACE_ADVANCE`] for anything without a cell.
pub fn text_width(font: &Font, text: &str) -> i32 {
    let mut width = 0;
    let mut glyphs: i32 = 0;
    for ch in text.chars() {
        if let Some(glyph) = glyph_of(font, ch) {
            width += (glyph.len() / font.rows() as usize) as i32;
            glyphs += 1;
        } else {
            width += SPACE_ADVANCE;
        }
    }
    width + (glyphs.saturating_sub(1) * GLYPH_GAP)
}

/// Draw `text` with its first glyph's cell at `(x, y)`, in `base`'s ink group.
pub fn draw_text(frame: &mut IndexedFrame, font: &Font, x: i32, y: i32, text: &str, base: u8) {
    let rows = font.rows() as usize;
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
pub(crate) mod tests {
    use super::*;

    /// The pixels of the `height` rows from `top`.
    pub(crate) fn band(frame: &IndexedFrame, top: i32, height: i32) -> Vec<u8> {
        let mut band = Vec::new();
        for y in top..top + height {
            band.extend_from_slice(frame.row(y as u16));
        }
        band
    }

    /// The leftmost and rightmost inked columns of `band` within `from..to`.
    pub(crate) fn ink_span(band: &[u8], width: u16, from: i32, to: i32) -> (Option<i32>, Option<i32>) {
        let columns: Vec<i32> = band
            .iter()
            .enumerate()
            .filter(|(_, &pixel)| pixel != 0)
            .map(|(at, _)| (at % width as usize) as i32)
            .filter(|&x| x >= from && x < to)
            .collect();
        (columns.iter().copied().min(), columns.iter().copied().max())
    }

    /// Assert that `frame` carries the ink of `text` drawn centred at `top` on its own.
    pub(crate) fn assert_centred(frame: &IndexedFrame, font: &Font, top: i32, text: &str, ink: u8) {
        let mut expected = IndexedFrame::new(frame.width, frame.height);
        draw_centered(&mut expected, font, top, text, ink);
        let drawn: Vec<usize> = expected
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, &pixel)| pixel != 0)
            .map(|(at, _)| at)
            .collect();
        assert!(!drawn.is_empty(), "{text} drew no ink");
        for at in drawn {
            assert_eq!(frame.pixels[at], expected.pixels[at], "{text} is missing");
        }
    }

    #[test]
    fn the_table_is_the_sources_order_up_to_each_cell() {
        // `ALKU/MAIN.C:40`. A cell's character is `'A' + its index` (`Font::glyph`).
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
        assert_eq!(FONA_ORDER.chars().count(), 74);
    }

    #[test]
    fn the_seven_marks_draw_ink_of_their_own() {
        let font = test_font();
        let mut seen = Vec::new();
        for mark in "()+-*='".chars() {
            let glyph = glyph_of(&font, mark).unwrap_or_else(|| panic!("{mark:?} has no cell"));
            let width = glyph.len() / font.rows() as usize;
            assert!((3..=25).contains(&width), "{mark:?} is {width} columns");
            assert!(glyph.iter().any(|&v| v != 0), "{mark:?} is blank");
            assert!(!seen.contains(&glyph), "{mark:?} draws the same glyph as an earlier mark");
            seen.push(glyph);
        }
    }

    #[test]
    fn a_space_is_the_only_advance_without_a_glyph() {
        let font = test_font();
        let word = text_width(&font, "A");
        assert_eq!(text_width(&font, "AA"), 2 * word + GLYPH_GAP);
        assert_eq!(text_width(&font, "A A"), 2 * word + GLYPH_GAP + SPACE_ADVANCE);
    }

    #[test]
    fn the_palette_is_six_bit_with_a_black_background() {
        let palette = menu_palette();
        assert_eq!(palette.entry(0), [0, 0, 0]);
        for base in [TITLE_INK, ENTRY_INK, SELECT_INK, FOOTER_INK] {
            for step in 0..3 {
                assert!(palette.entry(base + step).iter().all(|&v| v <= 63));
            }
        }
    }
}
