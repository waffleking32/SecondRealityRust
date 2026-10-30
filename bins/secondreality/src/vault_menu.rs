//! The vault menu: the pure state of the `--vault` screen and the frame it draws.
//!
//! GRID, HARD and TWIST are three standalone effects the demo never shipped
//! (`parts::vault`); `--vault` opens a window that selects among them. The
//! selection is a new surface and **not** a fidelity one: there is no original
//! menu frame to match, so this module invents the picture — but it draws its
//! text with the demo's own font, `ALKU/FONA.UH`, read by the same
//! [`Font`] the parts use, at 1:1 ([`crate::menu_text`], shared with the settings
//! screen).
//!
//! **The state is a reducer**, not a loop: [`Menu::key`] is a function of the
//! key and the current selection alone, so the menu's behaviour is testable
//! without a window. The window owns the loop and the clock; this module owns
//! nothing but the selection and the pixels.

use sr_assets::font::Font;
use sr_engine::IndexedFrame;

#[cfg(test)]
use crate::menu_text::text_width;
use crate::menu_text::{
    draw_centered, draw_marker, draw_text, menu_palette, ENTRY_INK, FOOTER_INK, SELECT_INK,
    TITLE_INK,
};
use crate::parts::vault::VAULT;

/// The menu's frame is the demo's own mode-13h geometry, but it is a plain
/// indexed frame — no surface, no view, no border.
pub const FRAME_WIDTH: u16 = 320;
pub const FRAME_HEIGHT: u16 = 200;

/// The row at which the first title line's ink starts.
const TITLE_TOP: i32 = 22;
/// The second title line.
const SUBTITLE_TOP: i32 = 46;
/// The first entry's ink, and the pitch to the next.
const ENTRY_TOP: i32 = 90;
const ENTRY_STEP: i32 = 22;
/// The selected row's `>` marker, and the column every label starts at.
const MARKER_X: i32 = 96;
const LABEL_X: i32 = 120;
/// The footer's ink.
const FOOTER_TOP: i32 = 172;

/// A key the menu acts on. Everything SDL reports that the menu does not use is
/// [`MenuKey::Other`], so the mapping is total and the reducer has no
/// "unknown" arm to leave open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Enter,
    Escape,
    Other,
}

/// What a key asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    /// The selection moved (or the key was ignored): draw the menu again.
    /// A redraw after an ignored key is identical to the frame before it.
    Redraw,
    /// Start this part — the registry name, not the label.
    Choose(&'static str),
    /// ESC in the menu: the window closes.
    Close,
}

/// The menu's selection: an index into [`VAULT`], in menu order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu {
    selected: usize,
}

impl Menu {
    /// The menu on GRID, the first row.
    pub fn new() -> Menu {
        Menu { selected: 0 }
    }

    /// The row [`Enter`](MenuKey::Enter) would start.
    fn selected_part(&self) -> &'static str {
        VAULT[self.selected].0
    }

    /// Apply one key.
    ///
    /// Up and Down wrap through the three rows; Enter starts the selected one;
    /// Escape closes the window. Anything else leaves the selection alone and
    /// asks for a redraw — which changes no pixel, because the picture is a
    /// pure function of the selection.
    pub fn key(&mut self, key: MenuKey) -> MenuAction {
        match key {
            MenuKey::Up => {
                self.selected = (self.selected + VAULT.len() - 1) % VAULT.len();
                MenuAction::Redraw
            }
            MenuKey::Down => {
                self.selected = (self.selected + 1) % VAULT.len();
                MenuAction::Redraw
            }
            MenuKey::Enter => MenuAction::Choose(self.selected_part()),
            MenuKey::Escape => MenuAction::Close,
            MenuKey::Other => MenuAction::Redraw,
        }
    }
}

impl Default for Menu {
    fn default() -> Menu {
        Menu::new()
    }
}

/// Draw the menu into `frame`.
///
/// The frame is cleared to the background first; its palette timeline is reset
/// to [`menu_palette`], so a caller can hand in the frame it made with
/// [`IndexedFrame::new`] and get the finished picture. The caller owns the
/// frame's size: the menu is laid out for 320×200 and clips everything else.
pub fn draw(menu: &Menu, font: &Font, frame: &mut IndexedFrame) {
    debug_assert_eq!(
        (frame.width, frame.height),
        (FRAME_WIDTH, FRAME_HEIGHT),
        "the vault menu is laid out for {FRAME_WIDTH}x{FRAME_HEIGHT}"
    );
    frame.pixels.fill(0);
    frame.palettes.reset(menu_palette());

    draw_centered(frame, font, TITLE_TOP, "SECOND REALITY", TITLE_INK);
    draw_centered(frame, font, SUBTITLE_TOP, "VAULT", TITLE_INK);

    for (row, (_, label)) in VAULT.iter().enumerate() {
        let y = ENTRY_TOP + row as i32 * ENTRY_STEP;
        if row == menu.selected {
            draw_marker(frame, MARKER_X, y + 2);
            draw_text(frame, font, LABEL_X, y, label, SELECT_INK);
        } else {
            draw_text(frame, font, LABEL_X, y, label, ENTRY_INK);
        }
    }

    draw_centered(frame, font, FOOTER_TOP, "UP DOWN  ENTER  ESC", FOOTER_INK);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_assets::load_path;
    use std::path::Path;

    /// `ALKU/FONA.UH`, scanned the way the demo's own text routine scans it.
    /// Unit tests run in the binary's directory.
    fn test_font() -> Font {
        let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).expect("the font strip");
        Font::from_image(image)
    }

    fn menu_frame(menu: &Menu, font: &Font) -> IndexedFrame {
        let mut frame = IndexedFrame::new(FRAME_WIDTH, FRAME_HEIGHT);
        draw(menu, font, &mut frame);
        frame
    }

    /// The pixels of one entry's row band.
    fn entry_band(frame: &IndexedFrame, row: usize) -> Vec<u8> {
        let top = ENTRY_TOP as u16 + row as u16 * ENTRY_STEP as u16;
        let mut band = Vec::new();
        for y in top..top + 14 {
            band.extend_from_slice(frame.row(y));
        }
        band
    }

    #[test]
    fn the_selection_wraps_up_and_down() {
        let mut menu = Menu::new();
        // Up from the first row wraps to the last: 0 → 2 → 1 → 0.
        for expected in [2, 1, 0] {
            assert_eq!(menu.key(MenuKey::Up), MenuAction::Redraw);
            assert_eq!(menu.selected, expected);
        }
        // And Down wraps the other way: 0 → 1 → 2 → 0.
        for expected in [1, 2, 0] {
            assert_eq!(menu.key(MenuKey::Down), MenuAction::Redraw);
            assert_eq!(menu.selected, expected);
        }
        // The order is the table's, which is the menu's own.
        assert_eq!(menu.selected_part(), "grid");
    }

    #[test]
    fn enter_chooses_the_selected_part() {
        let mut menu = Menu::new();
        assert_eq!(menu.key(MenuKey::Enter), MenuAction::Choose("grid"));
        menu.key(MenuKey::Down);
        assert_eq!(menu.key(MenuKey::Enter), MenuAction::Choose("hard"));
        menu.key(MenuKey::Down);
        assert_eq!(menu.key(MenuKey::Enter), MenuAction::Choose("twist"));
        // The chosen strings are the registry's names, not the labels: a menu
        // choice starts the same program `--part <name>` does.
        assert_eq!(MenuAction::Choose(VAULT[1].0), MenuAction::Choose("hard"));
    }

    #[test]
    fn escape_closes_the_menu() {
        assert_eq!(Menu::new().key(MenuKey::Escape), MenuAction::Close);
        // From any row, and without moving the selection.
        let mut menu = Menu::new();
        menu.key(MenuKey::Down);
        assert_eq!(menu.key(MenuKey::Escape), MenuAction::Close);
        assert_eq!(menu.selected, 1);
    }

    #[test]
    fn an_unhandled_key_redraws_nothing() {
        // The reducer still answers `Redraw` — the menu stays up — but a
        // redraw is a pure function of a selection this key did not move, so
        // the frame is byte-identical to the one before it.
        let font = test_font();
        let mut menu = Menu::new();
        let before = menu_frame(&menu, &font);
        assert_eq!(menu.key(MenuKey::Other), MenuAction::Redraw);
        let after = menu_frame(&menu, &font);
        assert_eq!(menu.selected, 0, "the selection did not move");
        assert_eq!(
            after.pixels, before.pixels,
            "an unhandled key changes no pixel"
        );
        assert_eq!(after.palettes, before.palettes);
    }

    #[test]
    fn the_menu_frame_is_three_entries_with_a_selected_row() {
        let font = test_font();
        let mut menu = Menu::new();
        let first = menu_frame(&menu, &font);

        assert_eq!((first.width, first.height), (FRAME_WIDTH, FRAME_HEIGHT));
        assert!(
            first.pixels.iter().any(|&pixel| pixel != 0),
            "the menu frame is blank"
        );
        assert_eq!(
            first.palettes.base(),
            &menu_palette(),
            "the frame carries the menu's palette"
        );

        // Three rows carry ink, and they are three different labels: GRID,
        // HARD and TWIST do not draw the same glyphs. Each row's ink starts at
        // the label column and runs exactly as wide as the strip's own glyphs
        // for that word — which is what makes this "the three labels" and not
        // just "three rows of something".
        for (row, (_, label)) in VAULT.iter().enumerate() {
            let band = entry_band(&first, row);
            assert!(band.iter().any(|&pixel| pixel != 0), "entry {row} is blank");
            let (mut left, mut right) = (usize::MAX, 0);
            for (at, &pixel) in band.iter().enumerate() {
                let x = at % first.width as usize;
                if pixel == 0 || x < LABEL_X as usize {
                    continue;
                }
                left = left.min(x);
                right = right.max(x);
            }
            assert_eq!(
                left, LABEL_X as usize,
                "{label} does not start at the label column"
            );
            assert_eq!(
                right,
                (LABEL_X + text_width(&font, label) - 1) as usize,
                "{label} is not drawn at the strip's own width"
            );
        }
        assert_ne!(entry_band(&first, 0), entry_band(&first, 1));
        assert_ne!(entry_band(&first, 1), entry_band(&first, 2));

        // The selected row is visibly different: the marker is added and the
        // label changes ink group. Moving the selection down leaves row 0
        // unmarked and marks row 1 instead, so row 0 changes and row 2 — not
        // selected in either frame — does not.
        menu.key(MenuKey::Down);
        let second = menu_frame(&menu, &font);
        assert_ne!(
            entry_band(&first, 0),
            entry_band(&second, 0),
            "the selected row does not look selected"
        );
        assert_eq!(
            entry_band(&first, 2),
            entry_band(&second, 2),
            "an unselected row changed when the selection moved"
        );

        // The selection's `>` marker is a specific glyph at MARKER_X, in the
        // selection's full ink; deleting it leaves every other assertion here
        // green, so it is pinned directly.
        let marker_top = (ENTRY_TOP + 2) as u16;
        let marker_at = |frame: &IndexedFrame, row: u16| {
            frame.pixels[row as usize * FRAME_WIDTH as usize + MARKER_X as usize]
        };
        assert_eq!(
            marker_at(&first, marker_top),
            SELECT_INK + 2,
            "the selected row has no `>` marker"
        );
        assert_eq!(
            marker_at(&second, marker_top),
            0,
            "row 0 keeps its marker after the selection moved"
        );
        assert_eq!(
            marker_at(&second, marker_top + ENTRY_STEP as u16),
            SELECT_INK + 2,
            "row 1's marker was not drawn"
        );
        // No unselected row draws anything left of the label column.
        for row in 1..VAULT.len() {
            let band = entry_band(&first, row);
            assert!(
                band.iter().enumerate().all(|(at, &pixel)| {
                    pixel == 0 || (at % first.width as usize) >= LABEL_X as usize
                }),
                "row {row} draws left of the label column"
            );
        }

        // The subtitle is the word `VAULT`, centred under the title; compare
        // its ink with a separately drawn copy, so deleting the subtitle line
        // fails here even though the rest of the frame is unchanged.
        let centered = (FRAME_WIDTH as i32 - text_width(&font, "VAULT")) / 2;
        let mut subtitle = IndexedFrame::new(FRAME_WIDTH, FRAME_HEIGHT);
        draw_text(
            &mut subtitle,
            &font,
            centered,
            SUBTITLE_TOP,
            "VAULT",
            TITLE_INK,
        );
        let subtitle_ink: Vec<usize> = subtitle
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, &pixel)| pixel != 0)
            .map(|(at, _)| at)
            .collect();
        assert!(!subtitle_ink.is_empty(), "the VAULT subtitle drew no ink");
        for at in subtitle_ink {
            assert_eq!(
                first.pixels[at],
                subtitle.pixels[at],
                "the VAULT subtitle is missing at ({}, {})",
                at % FRAME_WIDTH as usize,
                at / FRAME_WIDTH as usize
            );
        }

        // The title and footer are drawn as well as the entries.
        assert!(first.row(TITLE_TOP as u16).iter().any(|&pixel| pixel != 0));
        assert!(first.row(FOOTER_TOP as u16).iter().any(|&pixel| pixel != 0));
    }

    #[test]
    fn the_palette_is_six_bit_and_has_a_group_for_each_role() {
        let palette = menu_palette();
        assert_eq!(palette.entry(0), [0, 0, 0], "the background is black");
        // Every non-background entry the menu writes is inside 6 bits; the
        // three shades of a group get brighter.
        for (base, name) in [
            (TITLE_INK, "title"),
            (ENTRY_INK, "entry"),
            (SELECT_INK, "selection"),
            (FOOTER_INK, "footer"),
        ] {
            for step in 0..3 {
                let rgb = palette.entry(base + step);
                assert!(rgb.iter().all(|&v| v <= 63), "{name} {step} is not six bit");
                if step > 0 {
                    assert!(
                        palette.entry(base + step - 1).iter().sum::<u8>() <= rgb.iter().sum::<u8>(),
                        "{name}'s shades do not brighten"
                    );
                }
            }
        }
        // White title, neutral grey entries, amber selection, blue footer.
        assert_eq!(palette.entry(TITLE_INK + 2), [63, 63, 63]);
        let grey = palette.entry(ENTRY_INK + 2);
        assert!(
            grey[0] == grey[1] && grey[1] == grey[2],
            "entries are not grey"
        );
        let amber = palette.entry(SELECT_INK + 2);
        assert!(
            amber[0] > amber[1] && amber[1] > amber[2],
            "selection is not amber"
        );
        let blue = palette.entry(FOOTER_INK + 2);
        assert!(blue[2] > blue[0] && blue[2] > blue[1], "footer is not blue");
    }
}
