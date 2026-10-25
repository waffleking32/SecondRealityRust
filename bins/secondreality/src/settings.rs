//! The settings screen: the setup the original asked for every time it started, as the
//! port's own surface.
//!
//! `START.EXE` ran `menu()` on every start but `/gravis` (`MAIN/START.C:89-95`), and the
//! menu's rows were its soundcard, sound quality and demo looping (`MAIN/MENU.C:158-250`).
//! Spec §1 replaces `START.EXE`, so this is that screen for a windowed demo run, drawn in
//! the demo's own font ([`crate::menu_text`], the layer the vault menu shares) and
//! **not a fidelity surface**: there is no original frame to match, and it changes no
//! compared frame. **Nothing persists** — the original asked every time — and the command
//! line's flags are the initial values ([`Settings::from_cli`]).
//!
//! The five rows are the original's, then the port's: **Sound** (the soundcard row:
//! Off is `--no-audio`, and the player still runs), **Sound quality** (the mixer,
//! D43), **Demo looping**, **Aspect** and **Border** (`PresentOptions`).
//!
//! **The state is a reducer**, like the vault menu's: [`Setup::key`] is a function of the key
//! and the state alone, so the screen's behaviour is testable without a window. Up and Down
//! move and **clamp** (`MENU.C:158-159`: `if(menuy<1) menuy=1; if(menuy>3) menuy=3;`); Left
//! and Right change the value and **wrap** (`:161-168`, `if(m_looping<0) m_looping=1;
//! if(m_looping>1) m_looping=0;`). Enter starts the run, and ESC leaves without one.

use sr_assets::font::Font;
use sr_audio::Quality;
use sr_engine::present::{Aspect, PresentOptions};
use sr_engine::IndexedFrame;

use crate::cli::Cli;
use crate::menu_text::{
    draw_centered, draw_marker, draw_text, menu_palette, ENTRY_INK, FOOTER_INK, SELECT_INK,
    TITLE_INK,
};
use crate::shell::Looping;

/// The screen's frame: 16:10 like the demo's own 320×200, and wide enough for the font at its
/// own size — "Sound quality" and "Authentic" are 307 columns between them, and the footer is
/// 475. The width is a multiple of 5, as a 4:3 box needs, and it fits a 4:3 window of 624×468
/// (the demo's own 4:3 frame is 384×288) on a desktop 600 lines tall.
pub const FRAME_WIDTH: u16 = 520;
pub const FRAME_HEIGHT: u16 = 325;

/// The rows' labels, in order, and the two values each takes (the first is value 0).
const ROWS: [(&str, [&str; 2]); 5] = [
    ("Sound", ["On", "Off"]),
    ("Sound quality", ["Authentic", "Enhanced"]),
    ("Demo looping", ["Disabled", "Enabled"]),
    ("Aspect", ["4:3", "Square"]),
    ("Border", ["On", "Off"]),
];

/// The footer: the keys the screen answers to.
const FOOTER: &str = "UP DOWN  LEFT RIGHT  ENTER  ESC";

const TITLE_TOP: i32 = 22;
const SUBTITLE_TOP: i32 = 58;
/// The first row's ink, and the pitch to the next.
const ENTRY_TOP: i32 = 105;
const ENTRY_STEP: i32 = 32;
/// The `>` marker's column, and the columns a row's label and its value start at.
const MARKER_X: i32 = 77;
const LABEL_X: i32 = 105;
const VALUE_X: i32 = 300;
const FOOTER_TOP: i32 = 285;

/// What the screen asks of the run that follows it. The command line's flags are the
/// initial values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settings {
    /// The soundcard row: `false` is `--no-audio`.
    pub sound: bool,
    pub quality: Quality,
    /// The looping row: Enabled is [`Looping::On`], `--loop`.
    pub looping: bool,
    pub aspect: Aspect,
    pub border: bool,
}

impl Settings {
    /// The command line's own values, which are where the screen starts.
    ///
    /// `/gravis`'s [`Looping::NoEndScroller`] is not looping, but a run that has it never
    /// shows the screen (`Cli::setup` is cleared with it), so the mapping never sees it.
    pub fn from_cli(cli: &Cli) -> Settings {
        Settings {
            sound: !cli.no_audio,
            quality: cli.quality,
            looping: cli.looping == Looping::On,
            aspect: cli.aspect,
            border: cli.border,
        }
    }

    /// Write the choices into the command line the run is built from: `no_audio`,
    /// `quality`, `looping`, `aspect` and `border`.
    ///
    /// Looping is written as `On` or `Off`, and a `/gravis` run's `NoEndScroller` is left
    /// alone — that run never shows the screen, so there is no choice to override.
    pub fn apply(&self, cli: &mut Cli) {
        cli.no_audio = !self.sound;
        cli.quality = self.quality;
        if cli.looping != Looping::NoEndScroller {
            cli.looping = if self.looping {
                Looping::On
            } else {
                Looping::Off
            };
        }
        cli.aspect = self.aspect;
        cli.border = self.border;
    }

    /// How the window presents with these choices: Aspect and Border are the
    /// [`PresentOptions`] fields, and `scale` is the command line's.
    pub fn present_options(&self, scale: u32) -> PresentOptions {
        PresentOptions {
            aspect: self.aspect,
            scale,
            border: self.border,
        }
    }

    /// Which of a row's two values is chosen: 0 or 1, in [`ROWS`]' order.
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
            1 => {
                self.quality = if second {
                    Quality::Enhanced
                } else {
                    Quality::Authentic
                }
            }
            2 => self.looping = second,
            3 => {
                self.aspect = if second {
                    Aspect::SquarePixels
                } else {
                    Aspect::FourThree
                }
            }
            _ => self.border = !second,
        }
    }
}

/// A key the screen acts on. Everything SDL reports that it does not use is
/// [`SettingsKey::Other`], so the mapping is total.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Other,
}

/// What a key asks the window to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    /// Something moved or changed (or the key was ignored): draw the screen again, and
    /// present it with [`Settings::present_options`], which Aspect and Border change.
    Redraw,
    /// Enter: start the run with these settings.
    Start,
    /// ESC: leave without playing. Like `fatalexit`, the end screen is printed.
    Exit,
}

/// The screen's state: the settings and the selected row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Setup {
    pub settings: Settings,
    pub selected: usize,
}

impl Setup {
    /// The screen on its first row, with `settings` as the values.
    pub fn new(settings: Settings) -> Setup {
        Setup {
            settings,
            selected: 0,
        }
    }

    /// Apply one key.
    ///
    /// Up and Down **clamp** at the first and last row (`MENU.C:158-159`); Left and Right
    /// step the selected row's value and **wrap** (`:161-168`); Enter starts, Escape exits,
    /// and anything else redraws the same picture.
    pub fn key(&mut self, key: SettingsKey) -> SettingsAction {
        match key {
            SettingsKey::Up => {
                self.selected = self.selected.saturating_sub(1);
                SettingsAction::Redraw
            }
            SettingsKey::Down => {
                self.selected = (self.selected + 1).min(ROWS.len() - 1);
                SettingsAction::Redraw
            }
            SettingsKey::Left => self.step(-1),
            SettingsKey::Right => self.step(1),
            SettingsKey::Enter => SettingsAction::Start,
            SettingsKey::Escape => SettingsAction::Exit,
            SettingsKey::Other => SettingsAction::Redraw,
        }
    }

    fn step(&mut self, by: isize) -> SettingsAction {
        let value = wrap(self.settings.value(self.selected), ROWS[self.selected].1.len(), by);
        self.settings.set_value(self.selected, value);
        SettingsAction::Redraw
    }
}

/// `value + by`, wrapped into `0..count` (`MENU.C:161-168`).
fn wrap(value: usize, count: usize, by: isize) -> usize {
    (value as isize + by).rem_euclid(count as isize) as usize
}

/// Draw the screen into `frame`: the title, the five rows with their values, the `>` on
/// the `selected` row, and the footer.
///
/// The frame is cleared first and its palette timeline reset to [`menu_palette`], as the
/// vault menu's is; it is laid out for [`FRAME_WIDTH`]×[`FRAME_HEIGHT`] and clips anything
/// else. Its overscan colour is set, so that the Border row has a border to show.
pub fn draw(settings: &Settings, selected: usize, frame: &mut IndexedFrame, font: &Font) {
    debug_assert_eq!(
        (frame.width, frame.height),
        (FRAME_WIDTH, FRAME_HEIGHT),
        "the settings screen is laid out for {FRAME_WIDTH}x{FRAME_HEIGHT}"
    );
    frame.pixels.fill(0);
    frame.palettes.reset(menu_palette());
    frame.border = Some(FOOTER_INK);

    draw_centered(frame, font, TITLE_TOP, "SECOND REALITY", TITLE_INK);
    draw_centered(frame, font, SUBTITLE_TOP, "SETUP", TITLE_INK);

    for (row, (label, values)) in ROWS.iter().enumerate() {
        let y = ENTRY_TOP + row as i32 * ENTRY_STEP;
        let (label_ink, value_ink) = if row == selected {
            draw_marker(frame, MARKER_X, y + 2);
            (SELECT_INK, SELECT_INK)
        } else {
            (ENTRY_INK, TITLE_INK)
        };
        draw_text(frame, font, LABEL_X, y, label, label_ink);
        draw_text(frame, font, VALUE_X, y, values[settings.value(row)], value_ink);
    }

    draw_centered(frame, font, FOOTER_TOP, FOOTER, FOOTER_INK);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_text::text_width;
    use sr_assets::load_path;
    use std::path::Path;

    fn test_font() -> Font {
        let image = load_path(Path::new("../../assets/ALKU/FONA.UH")).expect("the font strip");
        Font::from_image(image, (20, 30), 'A').expect("the strip read as ALKU reads it")
    }

    fn start() -> Setup {
        Setup::new(Settings::from_cli(&Cli::default()))
    }

    fn parse(args: &[&str]) -> Cli {
        Cli::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    fn frame_of(setup: &Setup, font: &Font) -> IndexedFrame {
        let mut frame = IndexedFrame::new(FRAME_WIDTH, FRAME_HEIGHT);
        draw(&setup.settings, setup.selected, &mut frame, font);
        frame
    }

    /// The pixels of one row's band, from its first line of ink down eighteen lines (the
    /// strip's glyphs live in rows 0..18).
    fn band(frame: &IndexedFrame, top: i32) -> Vec<u8> {
        let mut band = Vec::new();
        for y in top..top + 18 {
            band.extend_from_slice(frame.row(y as u16));
        }
        band
    }

    #[test]
    fn the_selection_clamps_at_both_ends() {
        let mut setup = start();
        // Up from the first row stays on it: the original clamps (`MENU.C:158-159`), the
        // vault menu's wrap is not this screen's rule.
        assert_eq!(setup.key(SettingsKey::Up), SettingsAction::Redraw);
        assert_eq!(setup.selected, 0);
        for expected in [1, 2, 3, 4, 4, 4] {
            assert_eq!(setup.key(SettingsKey::Down), SettingsAction::Redraw);
            assert_eq!(setup.selected, expected);
        }
        for expected in [3, 2, 1, 0, 0] {
            setup.key(SettingsKey::Up);
            assert_eq!(setup.selected, expected);
        }
    }

    #[test]
    fn left_and_right_wrap_each_value() {
        for (row, (label, _)) in ROWS.iter().enumerate() {
            let mut setup = start();
            setup.selected = row;
            let first = setup.settings;
            // Right off the second value wraps to the first, and so does Left off the
            // first to the second (`MENU.C:161-168`): two values, so each key flips.
            assert_eq!(setup.key(SettingsKey::Right), SettingsAction::Redraw);
            assert_eq!(setup.settings.value(row), 1, "{}: right from 0", label);
            assert_ne!(setup.settings, first);
            setup.key(SettingsKey::Right);
            assert_eq!(setup.settings, first, "{}: right wraps to 0", label);
            setup.key(SettingsKey::Left);
            assert_eq!(setup.settings.value(row), 1, "{}: left wraps to 1", label);
            setup.key(SettingsKey::Left);
            assert_eq!(setup.settings, first, "{}: left from 1", label);
            // Only the selected row changed.
            for other in (0..ROWS.len()).filter(|&other| other != row) {
                assert_eq!(setup.settings.value(other), first.value(other));
            }
        }
        // The wrap itself, for a row of any width.
        assert_eq!(wrap(0, 3, -1), 2);
        assert_eq!(wrap(2, 3, 1), 0);
        assert_eq!(wrap(1, 2, 1), 0);
        assert_eq!(wrap(0, 2, -1), 1);
    }

    #[test]
    fn enter_starts_and_escape_exits() {
        let mut setup = start();
        assert_eq!(setup.key(SettingsKey::Enter), SettingsAction::Start);
        // From any row, and without moving the selection or a value.
        setup.selected = 3;
        let before = setup;
        assert_eq!(setup.key(SettingsKey::Escape), SettingsAction::Exit);
        assert_eq!(setup.key(SettingsKey::Enter), SettingsAction::Start);
        assert_eq!(setup, before);
        // Any other key redraws the same state.
        assert_eq!(setup.key(SettingsKey::Other), SettingsAction::Redraw);
        assert_eq!(setup, before);
    }

    #[test]
    fn the_settings_frame_draws_five_rows_and_the_marker() {
        let font = test_font();
        let mut setup = start();
        let first = frame_of(&setup, &font);
        assert_eq!((first.width, first.height), (FRAME_WIDTH, FRAME_HEIGHT));
        assert_eq!(first.palettes.base(), &menu_palette());
        assert!(first.border.is_some(), "the Border row needs a border to show");

        // Each row carries its label at the label column and its value at the value
        // column, each exactly as wide as the strip's own glyphs make it.
        for (row, (label, values)) in ROWS.iter().enumerate() {
            let top = ENTRY_TOP + row as i32 * ENTRY_STEP;
            let band = band(&first, top);
            let ink = |from: i32, to: i32| {
                let columns: Vec<i32> = band
                    .iter()
                    .enumerate()
                    .filter(|(_, &pixel)| pixel != 0)
                    .map(|(at, _)| (at % FRAME_WIDTH as usize) as i32)
                    .filter(|&x| x >= from && x < to)
                    .collect();
                (columns.iter().copied().min(), columns.iter().copied().max())
            };
            assert_eq!(
                ink(LABEL_X, VALUE_X),
                (Some(LABEL_X), Some(LABEL_X + text_width(&font, label) - 1)),
                "row {row}: {label}"
            );
            let value = values[0];
            assert_eq!(
                ink(VALUE_X, FRAME_WIDTH as i32),
                (Some(VALUE_X), Some(VALUE_X + text_width(&font, value) - 1)),
                "row {row}: {value}"
            );
        }
        // Five rows, and five different pictures.
        for row in 1..ROWS.len() {
            assert_ne!(
                band(&first, ENTRY_TOP + (row as i32 - 1) * ENTRY_STEP),
                band(&first, ENTRY_TOP + row as i32 * ENTRY_STEP)
            );
        }

        // The `>` is on the selected row only, in the selection's full ink, and moves
        // with it.
        let marker = |frame: &IndexedFrame, row: i32| {
            frame.pixels[((ENTRY_TOP + row * ENTRY_STEP + 2) as usize) * FRAME_WIDTH as usize
                + MARKER_X as usize]
        };
        for row in 0..ROWS.len() as i32 {
            let expected = if row == 0 { SELECT_INK + 2 } else { 0 };
            assert_eq!(marker(&first, row), expected, "row {row} before moving");
        }
        setup.key(SettingsKey::Down);
        let second = frame_of(&setup, &font);
        assert_eq!(marker(&second, 0), 0);
        assert_eq!(marker(&second, 1), SELECT_INK + 2);
        // The selected row's ink changed group; an untouched row did not change.
        assert_ne!(band(&first, ENTRY_TOP), band(&second, ENTRY_TOP));
        assert_eq!(
            band(&first, ENTRY_TOP + 3 * ENTRY_STEP),
            band(&second, ENTRY_TOP + 3 * ENTRY_STEP)
        );

        // A changed value changes its row's picture, and only that row's.
        setup.key(SettingsKey::Right);
        let third = frame_of(&setup, &font);
        assert_ne!(
            band(&second, ENTRY_TOP + ENTRY_STEP),
            band(&third, ENTRY_TOP + ENTRY_STEP),
            "Authentic is not drawn as Enhanced"
        );
        assert_eq!(
            band(&second, ENTRY_TOP + 2 * ENTRY_STEP),
            band(&third, ENTRY_TOP + 2 * ENTRY_STEP)
        );

        // The title, the subtitle `SETUP` and the footer, each against its own drawing.
        for (top, text, ink) in [
            (TITLE_TOP, "SECOND REALITY", TITLE_INK),
            (SUBTITLE_TOP, "SETUP", TITLE_INK),
            (FOOTER_TOP, FOOTER, FOOTER_INK),
        ] {
            let mut expected = IndexedFrame::new(FRAME_WIDTH, FRAME_HEIGHT);
            draw_centered(&mut expected, &font, top, text, ink);
            let drawn: Vec<usize> = expected
                .pixels
                .iter()
                .enumerate()
                .filter(|(_, &pixel)| pixel != 0)
                .map(|(at, _)| at)
                .collect();
            assert!(!drawn.is_empty(), "{text} drew no ink");
            for at in drawn {
                assert_eq!(first.pixels[at], expected.pixels[at], "{text} is missing");
            }
        }
        // The footer is the whole key line, and it fits.
        assert!(text_width(&font, FOOTER) < FRAME_WIDTH as i32);
    }

    /// D44's widths, which are why the frame is not the demo's 320 columns: the widest label
    /// and value together, and the footer, at the font's own size.
    #[test]
    fn the_frame_is_sized_for_the_widest_row_and_the_footer() {
        let font = test_font();
        // Each row's label and its widest value, measured over every row the screen has.
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
        let footer = text_width(&font, FOOTER);
        assert_eq!(footer, 475, "the footer");
        assert!(footer > 320, "the footer does not fit the demo's 320 columns");
    }

    #[test]
    fn the_chosen_settings_are_the_runs_settings() {
        // The flags are the initial values...
        let cli = parse(&["--no-audio", "--sound-quality", "enhanced", "--loop",
                          "--aspect", "square", "--no-border"]);
        let from = Settings::from_cli(&cli);
        assert_eq!(
            from,
            Settings {
                sound: false,
                quality: Quality::Enhanced,
                looping: true,
                aspect: Aspect::SquarePixels,
                border: false,
            }
        );
        assert_eq!(Settings::from_cli(&Cli::default()).value(0), 0, "sound is on by default");

        // ...and every row's value reaches the run, from the other end of each.
        let mut cli = parse(&["--scale", "2"]);
        let mut setup = Setup::new(Settings::from_cli(&cli));
        for row in 0..ROWS.len() {
            setup.selected = row;
            setup.key(SettingsKey::Right);
        }
        setup.settings.apply(&mut cli);
        assert!(cli.no_audio, "Sound Off is --no-audio");
        assert_eq!(cli.quality, Quality::Enhanced);
        assert_eq!(cli.looping, Looping::On);
        assert_eq!(cli.aspect, Aspect::SquarePixels);
        assert!(!cli.border);
        let run = cli.run_settings();
        assert_eq!(run.player.quality, Quality::Enhanced, "the run's player is mixed as chosen");
        assert_eq!(run.player.output_rate(), 48_000, "so the card opens at the enhanced rate");
        assert_eq!(run.looping, Looping::On);
        assert_eq!(
            setup.settings.present_options(cli.scale),
            PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: false }
        );

        // And back: the other value of every row.
        for row in 0..ROWS.len() {
            setup.selected = row;
            setup.key(SettingsKey::Left);
        }
        setup.settings.apply(&mut cli);
        assert!(!cli.no_audio);
        assert_eq!(cli.quality, Quality::Authentic);
        assert_eq!(cli.looping, Looping::Off);
        assert_eq!(cli.run_settings().looping, Looping::Off);
        assert_eq!(cli.run_settings().player.output_rate(), 20_000);
        assert_eq!(
            setup.settings.present_options(cli.scale),
            PresentOptions { aspect: Aspect::FourThree, scale: 2, border: true }
        );

        // `/gravis`'s `NoEndScroller` is not the screen's to rewrite: that run never
        // shows it.
        let mut gravis = parse(&["/gravis"]);
        Settings::from_cli(&gravis).apply(&mut gravis);
        assert_eq!(gravis.looping, Looping::NoEndScroller);
    }
}
