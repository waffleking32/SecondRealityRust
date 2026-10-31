//! The `--vault` menu (D40): a selection among GRID, HARD and TWIST, the three effects the
//! demo never shipped (`parts::vault`). Its picture is the port's own, 320×200.

use sr_assets::font::Font;
use sr_engine::IndexedFrame;

use super::{draw_screen, Key, Layout};
use crate::parts::vault::VAULT;

pub const LAYOUT: Layout = Layout {
    width: 320,
    height: 200,
    subtitle: "VAULT",
    title_top: 22,
    subtitle_top: 46,
    entry_top: 90,
    entry_step: 22,
    marker_x: 96,
    label_x: 120,
    value_x: 0,
    footer_top: 172,
    footer: "UP DOWN  ENTER  ESC",
    border: None,
};

/// What a key asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Draw the menu again (an ignored key redraws the same picture).
    Redraw,
    /// Start this part: the registry name, not the label.
    Choose(&'static str),
    /// ESC: the window closes.
    Close,
}

/// The selection: an index into [`VAULT`], in menu order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Menu {
    selected: usize,
}

impl Menu {
    /// Apply one key: Up and Down wrap through the rows, Enter starts the selected one,
    /// Escape closes, and anything else redraws.
    pub fn key(&mut self, key: Key) -> Action {
        match key {
            Key::Up => {
                self.selected = (self.selected + VAULT.len() - 1) % VAULT.len();
                Action::Redraw
            }
            Key::Down => {
                self.selected = (self.selected + 1) % VAULT.len();
                Action::Redraw
            }
            Key::Enter => Action::Choose(VAULT[self.selected].0),
            Key::Escape => Action::Close,
            Key::Left | Key::Right | Key::Other => Action::Redraw,
        }
    }

    /// Draw the menu into a frame of [`LAYOUT`]'s size.
    pub fn draw(&self, font: &Font, frame: &mut IndexedFrame) {
        let rows = VAULT.iter().map(|&(_, label)| (label, None));
        draw_screen(frame, font, &LAYOUT, rows, self.selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::{assert_centred, band, ink_span};
    use crate::menu::{test_font, text_width, FOOTER_INK, SELECT_INK, TITLE_INK};

    fn frame_of(menu: &Menu, font: &Font) -> IndexedFrame {
        let mut frame = IndexedFrame::new(LAYOUT.width, LAYOUT.height);
        menu.draw(font, &mut frame);
        frame
    }

    fn row_band(frame: &IndexedFrame, row: usize) -> Vec<u8> {
        band(frame, LAYOUT.row_top(row), 14)
    }

    #[test]
    fn up_and_down_wrap_and_enter_chooses_the_registry_name() {
        let mut menu = Menu::default();
        assert_eq!(menu.key(Key::Enter), Action::Choose("grid"));
        for (key, expected) in [
            (Key::Up, "twist"),
            (Key::Up, "hard"),
            (Key::Up, "grid"),
            (Key::Down, "hard"),
            (Key::Down, "twist"),
            (Key::Down, "grid"),
        ] {
            assert_eq!(menu.key(key), Action::Redraw);
            assert_eq!(menu.key(Key::Enter), Action::Choose(expected), "{key:?}");
        }
    }

    #[test]
    fn escape_closes_and_other_keys_move_nothing() {
        let mut menu = Menu::default();
        menu.key(Key::Down);
        let font = test_font();
        let before = frame_of(&menu, &font);
        for key in [Key::Left, Key::Right, Key::Other] {
            assert_eq!(menu.key(key), Action::Redraw, "{key:?}");
        }
        assert_eq!(menu.selected, 1);
        assert_eq!(frame_of(&menu, &font).pixels, before.pixels);
        assert_eq!(menu.key(Key::Escape), Action::Close);
        assert_eq!(menu.selected, 1);
    }

    #[test]
    fn the_menu_frame_is_three_labels_with_a_marked_row() {
        let font = test_font();
        let mut menu = Menu::default();
        let first = frame_of(&menu, &font);
        assert!(first.border.is_none());

        // Each label starts at the label column and is exactly the strip's width for it,
        // and nothing is drawn left of it on an unselected row.
        for (row, (_, label)) in VAULT.iter().enumerate() {
            let band = row_band(&first, row);
            assert_eq!(
                ink_span(&band, LAYOUT.width, LAYOUT.label_x, LAYOUT.width as i32),
                (Some(LAYOUT.label_x), Some(LAYOUT.label_x + text_width(&font, label) - 1)),
                "{label}"
            );
            if row > 0 {
                assert_eq!(ink_span(&band, LAYOUT.width, 0, LAYOUT.label_x), (None, None));
            }
        }

        // The `>` moves with the selection, and only the rows it leaves and reaches change.
        let marker = |frame: &IndexedFrame, row: usize| {
            frame.pixels[(LAYOUT.row_top(row) + 2) as usize * LAYOUT.width as usize
                + LAYOUT.marker_x as usize]
        };
        assert_eq!(marker(&first, 0), SELECT_INK + 2);
        menu.key(Key::Down);
        let second = frame_of(&menu, &font);
        assert_eq!((marker(&second, 0), marker(&second, 1)), (0, SELECT_INK + 2));
        assert_ne!(row_band(&first, 0), row_band(&second, 0));
        assert_eq!(row_band(&first, 2), row_band(&second, 2));

        for (top, text, ink) in [
            (LAYOUT.title_top, "SECOND REALITY", TITLE_INK),
            (LAYOUT.subtitle_top, "VAULT", TITLE_INK),
            (LAYOUT.footer_top, LAYOUT.footer, FOOTER_INK),
        ] {
            assert_centred(&first, &font, top, text, ink);
        }
    }
}
