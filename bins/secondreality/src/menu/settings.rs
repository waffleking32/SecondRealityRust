//! The settings screen (D44): the setup `START.EXE` ran on every start but `/gravis`
//! (`MAIN/START.C:89-95`), as the port's own surface for a windowed demo run.
//!
//! The rows are the original's soundcard, sound quality and demo looping
//! (`MAIN/MENU.C:158-250`), then the port's Aspect and Border. The command line's flags are
//! the initial values ([`Settings::from_cli`]) and nothing persists.

use sr_assets::font::Font;
use sr_audio::Quality;
use sr_engine::present::{Aspect, PresentOptions};
use sr_engine::IndexedFrame;

use super::{draw_screen, Key, Layout, FOOTER_INK};
use crate::cli::Cli;
use crate::shell::Looping;

/// 16:10 like the demo's 320×200, and wide enough for the font at its own size: "Sound
/// quality" with "Authentic" is 307 columns and the footer 475. The width is a multiple of
/// 5, as a 4:3 box needs (D44).
pub const LAYOUT: Layout = Layout {
    width: 520,
    height: 325,
    subtitle: "SETUP",
    title_top: 22,
    subtitle_top: 58,
    entry_top: 105,
    entry_step: 32,
    marker_x: 77,
    label_x: 105,
    value_x: 300,
    footer_top: 285,
    footer: "UP DOWN  LEFT RIGHT  ENTER  ESC",
    // The Border row needs a border to show.
    border: Some(FOOTER_INK),
};

/// The rows' labels and their two values; value 0 is the default.
const ROWS: [(&str, [&str; 2]); 5] = [
    ("Sound", ["On", "Off"]),
    ("Sound quality", ["Authentic", "Enhanced"]),
    ("Demo looping", ["Disabled", "Enabled"]),
    ("Aspect", ["4:3", "Square"]),
    ("Border", ["On", "Off"]),
];

/// What the screen asks of the run that follows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The soundcard row: `false` is `--no-audio`.
    pub sound: bool,
    pub quality: Quality,
    /// Demo looping: Enabled is [`Looping::On`], `--loop`.
    pub looping: bool,
    pub aspect: Aspect,
    pub border: bool,
}

impl Settings {
    /// The command line's own values. A `/gravis` run never shows the screen, so its
    /// [`Looping::NoEndScroller`] never reaches this.
    pub fn from_cli(cli: &Cli) -> Settings {
        Settings {
            sound: !cli.no_audio,
            quality: cli.quality,
            looping: cli.looping == Looping::On,
            aspect: cli.aspect,
            border: cli.border,
        }
    }

    /// Write the choices into the command line the run is built from. A `/gravis` run's
    /// `NoEndScroller` is left alone.
    pub fn apply(&self, cli: &mut Cli) {
        cli.no_audio = !self.sound;
        cli.quality = self.quality;
        if cli.looping != Looping::NoEndScroller {
            cli.looping = if self.looping { Looping::On } else { Looping::Off };
        }
        cli.aspect = self.aspect;
        cli.border = self.border;
    }

    /// How the window presents with these choices, at the command line's `scale`.
    pub fn present_options(&self, scale: u32) -> PresentOptions {
        PresentOptions { aspect: self.aspect, scale, border: self.border }
    }

    /// Which of a row's two values is chosen, in [`ROWS`]' order.
    fn value(&self, row: usize) -> usize {
        match row {
            0 => !self.sound as usize,
            1 => (self.quality == Quality::Enhanced) as usize,
            2 => self.looping as usize,
            3 => (self.aspect == Aspect::SquarePixels) as usize,
            _ => !self.border as usize,
        }
    }

    fn set_value(&mut self, row: usize, value: usize) {
        let second = value == 1;
        match row {
            0 => self.sound = !second,
            1 => self.quality = if second { Quality::Enhanced } else { Quality::Authentic },
            2 => self.looping = second,
            3 => self.aspect = if second { Aspect::SquarePixels } else { Aspect::FourThree },
            _ => self.border = !second,
        }
    }
}

/// What a key asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Draw the screen again and present it under [`Settings::present_options`].
    Redraw,
    /// Enter: start the run with these settings.
    Start,
    /// ESC: leave without playing (the end box is printed, as `fatalexit` does).
    Exit,
}

/// The screen's state: the settings and the selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub settings: Settings,
    pub selected: usize,
}

impl Setup {
    /// The screen on its first row.
    pub fn new(settings: Settings) -> Setup {
        Setup { settings, selected: 0 }
    }

    /// Apply one key. Up and Down **clamp** (`MENU.C:158-159`: `if(menuy<1) menuy=1;
    /// if(menuy>3) menuy=3;`); Left and Right step the value and **wrap** (`:161-168`:
    /// `if(m_looping<0) m_looping=1; if(m_looping>1) m_looping=0;`).
    pub fn key(&mut self, key: Key) -> Action {
        match key {
            Key::Up => self.selected = self.selected.saturating_sub(1),
            Key::Down => self.selected = (self.selected + 1).min(ROWS.len() - 1),
            Key::Left => self.step(-1),
            Key::Right => self.step(1),
            Key::Enter => return Action::Start,
            Key::Escape => return Action::Exit,
            Key::Other => {}
        }
        Action::Redraw
    }

    fn step(&mut self, by: isize) {
        let count = ROWS[self.selected].1.len() as isize;
        let value = (self.settings.value(self.selected) as isize + by).rem_euclid(count);
        self.settings.set_value(self.selected, value as usize);
    }

    /// Draw the screen into a frame of [`LAYOUT`]'s size.
    pub fn draw(&self, font: &Font, frame: &mut IndexedFrame) {
        let rows = ROWS
            .iter()
            .enumerate()
            .map(|(row, (label, values))| (*label, Some(values[self.settings.value(row)])));
        draw_screen(frame, font, &LAYOUT, rows, self.selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::tests::{assert_centred, band, ink_span};
    use crate::menu::{menu_palette, test_font, text_width, SELECT_INK, TITLE_INK};
    use sr_engine::present::output_size;

    fn start() -> Setup {
        Setup::new(Settings::from_cli(&Cli::default()))
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    fn frame_of(setup: &Setup, font: &Font) -> IndexedFrame {
        let mut frame = IndexedFrame::new(LAYOUT.width, LAYOUT.height);
        setup.draw(font, &mut frame);
        frame
    }

    /// One row's band: the strip's glyphs live in rows 0..18.
    fn row_band(frame: &IndexedFrame, row: usize) -> Vec<u8> {
        band(frame, LAYOUT.row_top(row), 18)
    }

    #[test]
    fn the_selection_clamps_at_both_ends() {
        let mut setup = start();
        assert_eq!(setup.key(Key::Up), Action::Redraw);
        assert_eq!(setup.selected, 0);
        for expected in [1, 2, 3, 4, 4, 4] {
            assert_eq!(setup.key(Key::Down), Action::Redraw);
            assert_eq!(setup.selected, expected);
        }
        for expected in [3, 2, 1, 0, 0] {
            setup.key(Key::Up);
            assert_eq!(setup.selected, expected);
        }
    }

    #[test]
    fn left_and_right_wrap_each_value_and_only_that_row_changes() {
        for (row, (label, _)) in ROWS.iter().enumerate() {
            let mut setup = start();
            setup.selected = row;
            let first = setup.settings;
            assert_eq!(setup.key(Key::Right), Action::Redraw);
            assert_eq!(setup.settings.value(row), 1, "{label}: right from 0");
            setup.key(Key::Right);
            assert_eq!(setup.settings, first, "{label}: right wraps to 0");
            setup.key(Key::Left);
            assert_eq!(setup.settings.value(row), 1, "{label}: left wraps to 1");
            for other in (0..ROWS.len()).filter(|&other| other != row) {
                assert_eq!(setup.settings.value(other), first.value(other));
            }
            setup.key(Key::Left);
            assert_eq!(setup.settings, first, "{label}: left from 1");
        }
    }

    #[test]
    fn enter_starts_escape_exits_and_neither_moves_anything() {
        let mut setup = start();
        setup.selected = 3;
        let before = setup;
        assert_eq!(setup.key(Key::Enter), Action::Start);
        assert_eq!(setup.key(Key::Escape), Action::Exit);
        assert_eq!(setup.key(Key::Other), Action::Redraw);
        assert_eq!(setup, before);
    }

    #[test]
    fn the_settings_frame_draws_five_rows_and_the_marker() {
        let font = test_font();
        let mut setup = start();
        let first = frame_of(&setup, &font);
        assert_eq!(first.palettes.base(), &menu_palette());
        assert!(first.border.is_some(), "the Border row needs a border to show");

        // Each row's label and value start at their columns at the strip's own widths.
        for (row, (label, values)) in ROWS.iter().enumerate() {
            let band = row_band(&first, row);
            let span = |from, to| ink_span(&band, LAYOUT.width, from, to);
            let (label_x, value_x) = (LAYOUT.label_x, LAYOUT.value_x);
            assert_eq!(
                span(label_x, value_x),
                (Some(label_x), Some(label_x + text_width(&font, label) - 1)),
                "{label}"
            );
            assert_eq!(
                span(value_x, LAYOUT.width as i32),
                (Some(value_x), Some(value_x + text_width(&font, values[0]) - 1)),
                "{label}: {}",
                values[0]
            );
            if row > 0 {
                assert_ne!(row_band(&first, row - 1), band);
            }
        }

        // The `>` is on the selected row only and moves with it.
        let marker = |frame: &IndexedFrame, row: usize| {
            frame.pixels[(LAYOUT.row_top(row) + 2) as usize * LAYOUT.width as usize
                + LAYOUT.marker_x as usize]
        };
        for row in 0..ROWS.len() {
            assert_eq!(marker(&first, row), if row == 0 { SELECT_INK + 2 } else { 0 });
        }
        setup.key(Key::Down);
        let second = frame_of(&setup, &font);
        assert_eq!((marker(&second, 0), marker(&second, 1)), (0, SELECT_INK + 2));
        assert_ne!(row_band(&first, 0), row_band(&second, 0));
        assert_eq!(row_band(&first, 3), row_band(&second, 3));

        // A changed value changes its row's picture, and only that row's.
        setup.key(Key::Right);
        let third = frame_of(&setup, &font);
        assert_ne!(row_band(&second, 1), row_band(&third, 1));
        assert_eq!(row_band(&second, 2), row_band(&third, 2));

        for (top, text, ink) in [
            (LAYOUT.title_top, "SECOND REALITY", TITLE_INK),
            (LAYOUT.subtitle_top, "SETUP", TITLE_INK),
            (LAYOUT.footer_top, LAYOUT.footer, FOOTER_INK),
        ] {
            assert_centred(&first, &font, top, text, ink);
        }
    }

    /// D44's widths, which are why the frame is not the demo's 320 columns.
    #[test]
    fn the_frame_is_sized_for_the_widest_row_and_the_footer() {
        let font = test_font();
        let widest = ROWS
            .iter()
            .map(|(label, values)| {
                let value = values.iter().map(|v| text_width(&font, v)).max().unwrap_or(0);
                text_width(&font, label) + value
            })
            .max()
            .expect("the screen has rows");
        let pair = text_width(&font, "Sound quality") + text_width(&font, "Authentic");
        assert_eq!((widest, pair), (307, 307), "the widest row is Sound quality / Authentic");
        assert_eq!(text_width(&font, LAYOUT.footer), 475, "the footer");
    }

    #[test]
    fn aspect_and_border_change_the_windows_raster() {
        // `Screen::present` resizes the window when `output_size` changes.
        let frame = frame_of(&start(), &test_font());
        let size = |aspect, border| {
            let settings = Settings { aspect, border, ..start().settings };
            output_size(&frame, &settings.present_options(1))
        };
        let square = size(Aspect::SquarePixels, true);
        let pad = (square.0 - 520) / 2;
        assert!(pad > 0, "the default run has its border");
        assert_eq!(square, (520 + 2 * pad, 325 + 2 * pad));
        assert_eq!(size(Aspect::FourThree, true), (624 + 2 * pad, 468 + 2 * pad));
        assert_eq!(size(Aspect::SquarePixels, false), (520, 325));
    }

    #[test]
    fn the_chosen_settings_are_the_runs_settings() {
        let cli = parse(&[
            "--no-audio", "--sound-quality", "enhanced", "--loop", "--aspect", "square",
            "--no-border",
        ]);
        assert_eq!(
            Settings::from_cli(&cli),
            Settings {
                sound: false,
                quality: Quality::Enhanced,
                looping: true,
                aspect: Aspect::SquarePixels,
                border: false,
            }
        );

        // Every row's other value reaches the run, and back.
        let mut cli = parse(&["--scale", "3"]);
        let mut setup = Setup::new(Settings::from_cli(&cli));
        for row in 0..ROWS.len() {
            setup.selected = row;
            setup.key(Key::Right);
        }
        setup.settings.apply(&mut cli);
        assert!(cli.no_audio, "Sound Off is --no-audio");
        assert_eq!((cli.quality, cli.looping), (Quality::Enhanced, Looping::On));
        assert_eq!((cli.aspect, cli.border), (Aspect::SquarePixels, false));
        let run = cli.run_settings();
        assert_eq!(run.player.output_rate(), 48_000, "the card opens at the enhanced rate");
        assert_eq!(run.looping, Looping::On);
        assert_eq!(setup.settings.present_options(cli.scale), cli.present_options());

        for row in 0..ROWS.len() {
            setup.selected = row;
            setup.key(Key::Left);
        }
        setup.settings.apply(&mut cli);
        assert!(!cli.no_audio);
        assert_eq!((cli.quality, cli.looping), (Quality::Authentic, Looping::Off));
        assert_eq!((cli.aspect, cli.border), (Aspect::FourThree, true));
        assert_eq!(cli.run_settings().player.output_rate(), 20_000);

        let mut gravis = parse(&["/gravis"]);
        Settings::from_cli(&gravis).apply(&mut gravis);
        assert_eq!(gravis.looping, Looping::NoEndScroller);
    }
}
