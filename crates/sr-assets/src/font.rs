//! Font strips: the glyphs a part draws its text from.
//!
//! `ALKU/FONA.UH` and `ENDSCRL/FONA.UH` (1500x30), and the art they were converted from
//! (`ALKU/FONA.LBM`, `ALKU/FONA2.LBM`, `ENDSCRL/FONA.LBM`). A part links one (`ALKU/INCLUDE.ASM:17`,
//! `ENDSCRL/ASMYT.ASM:73`) and draws out of it.
//!
//! **A strip is not a grid.** Every part that draws from one *scans* it for ink:
//! `ENDSCRL/MAIN.C:105-122` walks the columns, tests whether any of the strip's rows holds ink
//! (`for(y=0;y<FONAY;y++) if(font[y][x]) break;`) and hands each blank-to-ink-to-blank run to
//! the next character of its order (`fonap`/`fonaw`). `ALKU/MAIN.C:214-233` and
//! `CREDITS/MAIN.C:250-269` are the same loop. The runs are proportional (3..25 columns over
//! `ENDSCRL/FONA.UH`) and no uniform cell fits them: a 20-column cut, which `1500/20` invites,
//! splits 41 of the 74 glyphs (`tests/font.rs`).
//!
//! - A glyph is every row of the strip: the routine reads all of them for each character.
//! - The cells are in the part's order, starting at `A` (each `fonaorder` opens
//!   `ABCDEFGHIJKLMNOPQRSTUVWX`). The orders are not ASCII past `X` (`ENDSCRL/MAIN.C:13` goes on
//!   with `a`..`z`, digits, punctuation), so [`Font::glyph`] takes `'A' + index`, not a character.
//! - The blank is not a glyph: the routine places it by hand (`fonap[32]=1500-20`,
//!   `ENDSCRL/MAIN.C:123-124`, `ALKU/MAIN.C:234-235`) in columns that hold no ink.
//! - The pixels are the strip's own 2-bit values; the parts fold them themselves
//!   (`ALKU/MAIN.C:173-181`, `ENDSCRL/MAIN.C:80-87`).

use crate::Image;

/// The first cell's character: [`Font::glyph`] maps `FIRST + i` to cell `i`.
pub const FIRST: char = 'A';

/// A font strip: the ink runs of a decoded strip, repacked so each glyph is contiguous.
pub struct Font {
    /// Every glyph's pixels, row-major and back to back.
    glyphs: Vec<u8>,
    /// One entry per glyph, in the strip's order: where it starts in `glyphs` and its width.
    runs: Vec<(u32, u16)>,
    rows: u16,
}

impl Font {
    /// Scan `image` for its ink runs (`ENDSCRL/MAIN.C:105-117`). The image itself is not kept.
    pub fn from_image(image: Image) -> Font {
        let (width, rows) = (image.width() as usize, image.height() as usize);
        let pixels = image.pixels();
        let inked = |x: usize| (0..rows).any(|y| pixels[y * width + x] != 0);

        let mut glyphs = Vec::new();
        let mut runs = Vec::new();
        let mut x = 0;
        while x < width {
            if !inked(x) {
                x += 1;
                continue;
            }
            let start = x;
            while x < width && inked(x) {
                x += 1;
            }
            let at = glyphs.len() as u32;
            for y in 0..rows {
                glyphs.extend_from_slice(&pixels[y * width + start..y * width + x]);
            }
            runs.push((at, (x - start) as u16));
        }

        Font {
            glyphs,
            runs,
            rows: rows as u16,
        }
    }

    /// The glyph at index `ch as u32 - 'A' as u32`: [`Font::rows`] rows of the run's own width,
    /// row-major. `None` before `A` and past the last run. The index is the character's place
    /// in the part's order, not its ASCII code.
    pub fn glyph(&self, ch: char) -> Option<&[u8]> {
        let index = (ch as u32).checked_sub(FIRST as u32)? as usize;
        let &(at, width) = self.runs.get(index)?;
        let len = width as usize * self.rows as usize;
        Some(&self.glyphs[at as usize..at as usize + len])
    }

    /// Every glyph's height: the strip's.
    pub fn rows(&self) -> u16 {
        self.rows
    }
}

/// The geometry only, as [`Image`]'s.
impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("rows", &self.rows)
            .field("glyphs", &self.runs.len())
            .finish_non_exhaustive()
    }
}
