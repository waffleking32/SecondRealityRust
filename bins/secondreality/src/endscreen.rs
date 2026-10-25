//! The end screen: the box `SECOND.EXE` leaves on the DOS screen when it exits.
//!
//! `printend` (`MAIN/U2.ASM:506-535`) reads `U2END.BIN` — 4,000 bytes, 80×25 cells of a
//! CP437 character and its attribute — straight into text-mode memory at `B800:0000`
//! (`:522-529`) and puts the cursor on row 14, column 0 (`:530-533`), so DOS's prompt comes
//! back under the box. `fatalexit` calls it on **every** exit (`:974`), after the mode set
//! and the twenty blank `copperwaitborder`s (`:967-973`).
//!
//! The port's window is not a text-mode screen, so the stand-in is the terminal the run was
//! launched from (D41): the rows above the cursor, mapped from CP437 to Unicode, printed once
//! after the window closes — with the attributes as SGR colours when stdout is a terminal,
//! and as plain text when it is not.

/// The end screen, by the relative path every part reads its media by.
pub const END_SCREEN: &str = "assets/MAIN/DATA/U2END.BIN";

/// The row `printend` leaves the cursor on (`mov dx,256*14+0`, `MAIN/U2.ASM:532`): the
/// box is rows `0..CURSOR_ROW`, and what DOS prints next starts here.
pub const CURSOR_ROW: usize = 14;

/// Text mode 3's geometry: 80 columns, 25 rows, two bytes a cell.
const COLUMNS: usize = 80;
const ROWS: usize = 25;
const CELL: usize = 2;

/// The SGR reset every coloured line ends with, so the terminal's own colours come back.
const RESET: &str = "\x1b[0m";

const LOW: [char; 32] = [
    ' ', '☺', '☻', '♥', '♦', '♣', '♠', '•', // 0x00
    '◘', '○', '◙', '♂', '♀', '♪', '♫', '☼', // 0x08
    '►', '◄', '↕', '‼', '¶', '§', '▬', '↨', // 0x10
    '↑', '↓', '→', '←', '∟', '↔', '▲', '▼', // 0x18
];
const HIGH: [char; 128] = [
    'Ç', 'ü', 'é', 'â', 'ä', 'à', 'å', 'ç', // 0x80
    'ê', 'ë', 'è', 'ï', 'î', 'ì', 'Ä', 'Å', // 0x88
    'É', 'æ', 'Æ', 'ô', 'ö', 'ò', 'û', 'ù', // 0x90
    'ÿ', 'Ö', 'Ü', '¢', '£', '¥', '₧', 'ƒ', // 0x98
    'á', 'í', 'ó', 'ú', 'ñ', 'Ñ', 'ª', 'º', // 0xA0
    '¿', '⌐', '¬', '½', '¼', '¡', '«', '»', // 0xA8
    '░', '▒', '▓', '│', '┤', '╡', '╢', '╖', // 0xB0
    '╕', '╣', '║', '╗', '╝', '╜', '╛', '┐', // 0xB8
    '└', '┴', '┬', '├', '─', '┼', '╞', '╟', // 0xC0
    '╚', '╔', '╩', '╦', '╠', '═', '╬', '╧', // 0xC8
    '╨', '╤', '╥', '╙', '╘', '╒', '╓', '╫', // 0xD0
    '╪', '┘', '┌', '█', '▄', '▌', '▐', '▀', // 0xD8
    'α', 'ß', 'Γ', 'π', 'Σ', 'σ', 'µ', 'τ', // 0xE0
    'Φ', 'Θ', 'Ω', 'δ', '∞', 'φ', 'ε', '∩', // 0xE8
    '≡', '±', '≥', '≤', '⌠', '⌡', '÷', '≈', // 0xF0
    '°', '∙', '·', '√', 'ⁿ', '²', '■', '\u{a0}', // 0xF8
];

/// A CP437 byte as the glyph the VGA's ROM font draws for it.
///
/// `0x00` draws nothing, which is a space; `0x20..=0x7E` are ASCII; `0x7F` is the house.
/// The rest are the code page's own glyphs, `LOW` for the control range and `HIGH` for the
/// upper half (`0xFF` is the no-break space).
pub fn glyph(byte: u8) -> char {
    match byte {
        0x00..=0x1F => LOW[byte as usize],
        0x20..=0x7E => byte as char,
        0x7F => '⌂',
        0x80..=0xFF => HIGH[byte as usize - 0x80],
    }
}

/// Whether a cell draws no ink: a space, a NUL, or the no-break space.
fn is_blank(byte: u8) -> bool {
    matches!(byte, 0x00 | 0x20 | 0xFF)
}

/// The SGR sequence for a text-mode attribute: the low nibble is the foreground (bit 3
/// the bright half), bits 4–6 the background, and bit 7 — blink — is ignored.
///
/// The VGA's colour order is blue-green-red from bit 0 up and ANSI's is red-green-blue,
/// so the three colour bits are swapped end for end: VGA 1 (blue) is ANSI 4.
pub fn sgr(attribute: u8) -> String {
    const ANSI: [u8; 8] = [0, 4, 2, 6, 1, 5, 3, 7];
    let foreground = attribute & 0x0F;
    let background = (attribute >> 4) & 0x07;
    let fg = if foreground & 0x08 != 0 {
        90 + ANSI[(foreground & 0x07) as usize]
    } else {
        30 + ANSI[foreground as usize]
    };
    let bg = 40 + ANSI[background as usize];
    format!("\x1b[{fg};{bg}m")
}

/// Rows `0..CURSOR_ROW` of an 80×25 text screen as lines of text, each with its trailing
/// blanks trimmed.
///
/// With `ansi`, every change of attribute along a line — the first cell's included —
/// becomes an [`sgr`] sequence, and every line ends with `ESC[0m`. A `screen` shorter than
/// the rows asked for renders the rows it has.
pub fn render(screen: &[u8], ansi: bool) -> String {
    let mut out = String::new();
    for row in screen.chunks(COLUMNS * CELL).take(CURSOR_ROW.min(ROWS)) {
        let cells: Vec<(u8, u8)> = row.chunks_exact(CELL).map(|cell| (cell[0], cell[1])).collect();
        let inked = cells
            .iter()
            .rposition(|&(byte, _)| !is_blank(byte))
            .map_or(0, |last| last + 1);
        let mut attribute = None;
        for &(byte, cell_attribute) in &cells[..inked] {
            // Bit 7 is blink, which the port does not show: two attributes that
            // differ only there are the same colours.
            let colours = cell_attribute & 0x7F;
            if ansi && attribute != Some(colours) {
                out.push_str(&sgr(colours));
                attribute = Some(colours);
            }
            out.push(glyph(byte));
        }
        if ansi {
            out.push_str(RESET);
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped end screen, read where a unit test finds the repository's assets.
    fn shipped() -> Vec<u8> {
        let path = std::path::Path::new("../..").join(END_SCREEN);
        std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
    }

    #[test]
    fn the_end_screen_is_the_shipped_box() {
        let screen = shipped();
        assert_eq!(screen.len(), COLUMNS * ROWS * CELL, "80×25 cells of two bytes");
        let text = render(&screen, false);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), CURSOR_ROW, "the rows above printend's cursor");
        assert!(lines[0].starts_with('┌'), "{:?}", lines[0]);
        assert!(lines[0].ends_with('┐'), "{:?}", lines[0]);
        assert!(lines[1].contains("S E C O N D   R E A L I T Y"), "{:?}", lines[1]);
        assert!(lines[1].contains("Copyright (C) 1993 Future Crew"), "{:?}", lines[1]);
        assert!(lines[13].starts_with('└'), "{:?}", lines[13]);
        assert!(!text.contains('\x1b'), "plain text carries no escapes");
    }

    /// `printend` copies all 25 rows (`MAIN/U2.ASM:522-529`), and printing only the box is
    /// the same screen: rows 14–24 of the shipped file draw no ink (APPROXIMATIONS.md A59).
    #[test]
    fn the_rows_from_printends_cursor_down_are_blank() {
        let screen = shipped();
        for (row, cells) in screen.chunks(COLUMNS * CELL).enumerate().skip(CURSOR_ROW) {
            assert!(
                cells.chunks_exact(CELL).all(|cell| is_blank(cell[0])),
                "row {row} of U2END.BIN draws ink below the cursor"
            );
        }
    }

    #[test]
    fn the_end_screen_colours_are_its_attributes() {
        let screen = shipped();
        assert_eq!(screen[1], 0x08, "the box's frame is attribute 8");
        let text = render(&screen, true);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), CURSOR_ROW);
        // Attribute 0x08: foreground 8, bright black; background 0, black.
        assert_eq!(sgr(0x08), "\x1b[90;40m");
        assert!(
            lines[0].starts_with("\x1b[90;40m┌"),
            "line 0's first cell carries its attribute: {:?}",
            lines[0]
        );
        for line in &lines {
            assert!(line.ends_with(RESET), "every line ends with a reset: {line:?}");
        }
    }

    #[test]
    fn an_attribute_maps_its_nibbles_and_ignores_blink() {
        // VGA 1 is blue, ANSI 4; VGA 4 is red, ANSI 1.
        assert_eq!(sgr(0x01), "\x1b[34;40m");
        assert_eq!(sgr(0x0C), "\x1b[91;40m");
        assert_eq!(sgr(0x1F), "\x1b[97;44m", "bright white on blue");
        assert_eq!(sgr(0x9F), sgr(0x1F), "bit 7 is blink, and ignored");
        assert_eq!(sgr(0x70), "\x1b[30;47m");
    }

    #[test]
    fn the_code_page_maps_its_box_drawing_and_controls() {
        assert_eq!(glyph(0xDA), '┌');
        assert_eq!(glyph(0xBF), '┐');
        assert_eq!(glyph(0xC0), '└');
        assert_eq!(glyph(0xC4), '─');
        assert_eq!(glyph(0xB3), '│');
        assert_eq!(glyph(0x01), '☺');
        assert_eq!(glyph(0x00), ' ');
        assert_eq!(glyph(b'A'), 'A');
        assert_eq!(glyph(0x7F), '⌂');
        assert_eq!(glyph(0xFF), '\u{a0}');
    }
}
