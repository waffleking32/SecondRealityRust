//! Font strips: the glyph cells a part draws its text from.
//!
//! Five files in this tree are font strips — `ALKU/FONA.UH`,
//! `ENDSCRL/FONA.UH`, and the source art they were converted from,
//! `ALKU/FONA.LBM`, `ALKU/FONA2.LBM` and `ENDSCRL/FONA.LBM`. A part links one
//! of them (the `.UH`, or the `FONA.INC` `INCBIN` text of it:
//! `ALKU/INCLUDE.ASM:17`, `ENDSCRL/ASMYT.ASM:73`) and draws its text out of
//! it.
//!
//! **None of them is a grid.** That is the one thing this module exists to
//! get right, and it is a measurement, not a reading of the spec:
//!
//! - Every part that draws from a strip *scans* it for ink. `ENDSCRL/MAIN.C`
//!   walks `x` from 0 to 1500 one column at a time (`:109`, `:116`), tests
//!   whether any of the strip's rows holds ink in that column
//!   (`for(y=0;y<FONAY;y++) if(font[y][x]) break;`, `:107` and `:114`) and
//!   hands each blank-to-ink-to-blank run to the next character of its own
//!   order (`fonap`/`fonaw`, `:119-120`). `ALKU/MAIN.C:214-233` and
//!   `CREDITS/MAIN.C:250-269` are the same loop with `32` for `FONAY`.
//! - The result is proportional, and the strip's own bytes say so. Over
//!   `ENDSCRL/FONA.UH` the scan finds **74 ink runs whose widths run 3..25
//!   columns and whose start-to-start distances run 4..26** — and the file's
//!   own character order (`fonaorder`, `ENDSCRL/MAIN.C:13`) has exactly **74
//!   characters**, one per run, in order: run 0 is the 20-column `A`, run 24
//!   the x-height `a` (rows 4..13), run 30 the descender `g` (rows 3..17),
//!   run 60 the 3-column `!`. `ALKU/MAIN.C:40`'s order has 76 entries and
//!   `ALKU/FONA.UH` has 76 runs — the same strip layout, two more cells: the
//!   last two are 215 columns wide, the pictures that part prints as the
//!   characters `\x8f` and `\x99` (`ALKU/MAIN.C:75-76`).
//!
//! So there is no cell width to slice at, and picking one by arithmetic is
//! the trap this module's `Unsupported` arm is for. 1500 divides evenly by
//! 10, 15, 20, 25, 30 and 50; 10, 15 and 20 are narrower than the widest
//! glyph (25) and cut it, and 25, 30 and 50 give 60, 50 and 30 cells, all
//! fewer than the 74 characters the strip holds — so every uniform cell
//! either cuts a glyph in half or loses characters. The tempting 20 (`1500/20`
//! is exactly 74 + 1, the character count plus the blank) cuts **41 of the 74
//! glyphs**: the third cell, `C`, starts at column 37, inside the 20-column
//! cell the second cell's `B` lives in (`B` is 21..35). A font sliced that way
//! renders, and every test that only checks `glyph`'s length passes.
//!
//! What a part's routine *does* fix is the rest of the geometry, and that is
//! what [`Font::from_image`] takes:
//!
//! - **The rows.** Every glyph is exactly `FONAY` rows tall — the strip's own
//!   height — because the routine reads all of them for every character
//!   (`ENDSCRL/MAIN.C:6` defines `FONAY` as 30 and `:107`/`:114` scan that
//!   many rows; `ALKU/MAIN.C:218`/`:225` scan 32). That is `cell.1`.
//! - **The cell.** `cell.0` columns, which is the slot the routine reserves
//!   for its blank: `fonap[32]=1500-20` (`ENDSCRL/MAIN.C:123`,
//!   `ALKU/MAIN.C:234`) puts the blank's pixels in the strip's *last* 20
//!   columns, so the strip is a whole number of 20-column cells and the blank
//!   is the last of them. It agrees with the character table: 74 characters
//!   plus the blank is 75, and 1500/20 is 75. `CREDITS/MAIN.C:270-271` makes
//!   the same move with 32 and its raster (which no file in `assets/` holds)
//!   does not divide: 1500/32 is 46.875.
//! - **The characters.** `first` is the first cell's character — `A` for all
//!   three parts (each `fonaorder` opens `ABCDEFGHIJKLMNOPQRSTUVWX`). The
//!   orders are *not* ASCII-contiguous past `X` (`ENDSCRL/MAIN.C:13` runs
//!   `A`..`X`, then `a`..`z`, then the digits, then punctuation), so
//!   [`Font::glyph`] takes the cell's *index*, `ch as u32 - first as u32`
//!   — see its own docs.
//!
//! The parts disagree about the rows, and it is worth knowing which way.
//! `ALKU`'s routine reads 32 rows of a 30-row strip: the `fona.inc` fragment
//! is followed by `dw 1500*5 dup(?)` (`ALKU/INCLUDE.ASM:19`) — **7,500 words,
//! i.e. 15,000 bytes** of *uninitialised* padding, of which the two extra rows
//! the routine reads are 2 × 1,500 = 3,000 — that the linker filled with
//! something the port cannot know, and its glyphs live in rows 0..17 of the
//! strip's 30. What
//! exists is the strip, so `ALKU/FONA.UH` loads as 30 rows and a cell height
//! of 32 is refused — a cell that does not divide the strip, which is
//! [`Font::from_image`]'s other `Unsupported` arm.
//!
//! The pixel values are the strip's own: every glyph here is a 2-bit mask
//! (values 0..3 — the strip's whole value histogram). The parts turn those
//! into what they draw themselves: `ALKU/MAIN.C:173-181` folds `&3` onto the
//! top two bits of the screen byte, `ENDSCRL/MAIN.C:80-87` tests bit 0..3 of
//! each pixel into one of four EGA planes. That transform is the part's, not
//! the loader's, so [`Font::glyph`] hands back the strip's bytes unchanged.
//!
//! `GRAB/BLOWFNT.1BM` is not one of these files. It is 64,000 bytes of bare
//! 320×200 pixels ([`crate::raw_surface`]'s door, not this one), it is linked
//! into nothing — no `.OBJ` or `.EXE` in either tree holds any of its glyph
//! rows, and no part opens a `.1bm` (the extension is named only in
//! `GRAB/GFXSAVE.H:5`, a tool's comment) — and while its pixels do hold a grid
//! of 32×32 cells with rules at `x ≡ 31 (mod 32)` and `y ≡ 31 (mod 32)`, that
//! grid is ten cells by five and stops at row 159, leaving the canvas's last
//! 40 rows empty. No part's routine states its geometry, so no port code may
//! invent one.

use crate::{AssetError, Format, Image};

/// A font strip: the glyph cells one part's text is drawn from, repacked out
/// of the decoded image.
///
/// The cells are the runs of ink the part's own routine finds, in the strip's
/// order, which is the part's character order — one cell per character, and
/// every cell is `cell().1` rows tall.
///
/// The glyphs are *copied* out of the image, which is why they can be handed
/// out as `&[u8]` at all: a cell of a decoded `Surface` is not a contiguous
/// run of its pixels (the surface stores whole rows), so a view of one would
/// have to be a slice per row. The image itself is not kept — a strip's
/// palette is not part of a glyph, and a repacked strip is smaller than the
/// image it came from.
pub struct Font {
    /// Every cell's pixels, row-major and back to back.
    glyphs: Vec<u8>,
    /// One entry per cell, in the strip's order: where the cell starts in
    /// `glyphs` and how many columns wide it is.
    ///
    /// A width is a `u16` and cannot need more of one: it is at most the
    /// strip's own width, and every container this crate reads states its
    /// width in 16 bits ([`Image::width`] returns a `u16`), so no strip a
    /// decoder can hand [`Font::from_image`] is wider than 65,535 columns. The
    /// scan's run length is a plain cast for that reason; a strip that broke
    /// the bound is not representable in any of the tree's formats, so there
    /// is no case to refuse and nothing for a wider type to carry.
    cells: Vec<(u32, u16)>,
    cell: (u16, u16),
    first: char,
}

/// `raw_surface`'s own format name, for `raw_surface`'s own reason: a font
/// strip has no container here — the caller has already decoded it — and the
/// geometry it is read with is the caller's. Naming the picture's real format
/// would be a guess (`FONA.UH` is Origin and `FONA.LBM` is IFF, and the
/// `Image` a `Font` is built from no longer knows which it was), and no
/// `Format` variant is added for it: the phase's interface block fixes the
/// set, and a font strip is not a container [`crate::sniff`] could return.
const FORMAT: Format = Format::OriginBare;

fn unsupported(what: String) -> AssetError {
    AssetError::Unsupported {
        format: FORMAT,
        what,
    }
}

impl Font {
    /// A view of `image` as a strip of `cell.0`-column cells: `cell` is the
    /// cell geometry the part's own text routine states, and `first` is the
    /// character the first cell holds. Both are cited in `tests/font.rs`; see
    /// this module's docs for why they are the routine's and not a grid's.
    ///
    /// Two shapes are refused, both with [`AssetError::Unsupported`] (the
    /// variant's own doc: a construct no shipped file uses, where guessing
    /// would invent behaviour):
    ///
    /// - **A cell taller or shorter than the strip.** A strip is one line of
    ///   text: the routine reads every one of its rows for every character,
    ///   so the cell's height *is* the strip's height. `ALKU/FONA.UH` is 30
    ///   rows while `ALKU/MAIN.C`'s loop reads 32 — refused, because the two
    ///   rows past the strip are the linker's uninitialised padding.
    /// - **A strip that is not a whole number of cells across.** A partial
    ///   cell is a glyph cut in half; a font built from it would quietly lose
    ///   a character, so it is refused rather than truncated. (`CREDITS`'s
    ///   32-column cell does not divide its 1,500-column raster: 46.875.)
    ///
    /// A cell width that divides the strip but is not the routine's — 25 for
    /// `ENDSCRL` — is accepted, and the glyphs are unaffected: they come from
    /// the scan, never from the cell. Only [`Font::cell`]'s answer, and the
    /// two checks above, depend on what the caller states.
    pub fn from_image(image: Image, cell: (u16, u16), first: char) -> Result<Font, AssetError> {
        let (cw, rows) = (cell.0 as usize, cell.1 as usize);
        let (width, height) = (image.width() as usize, image.height() as usize);
        if cw == 0 || rows == 0 {
            return Err(unsupported(format!(
                "a {}x{} cell has no pixels in it",
                cell.0, cell.1
            )));
        }
        if rows != height {
            return Err(unsupported(format!(
                "the strip is {width} columns of {height} rows; its cell is one line of text, so it \
                 is {height} rows tall, not {}",
                cell.1
            )));
        }
        let leftover = width % cw;
        if leftover != 0 {
            return Err(unsupported(format!(
                "{width} columns is not a whole number of {cw}-column cells: the last cell is \
                 {leftover} columns of one, and the glyph in it would be cut in half"
            )));
        }

        // The scan the parts run, on the strip's own bytes: a cell is a run of
        // columns that hold ink, bounded by columns that hold none
        // (`ENDSCRL/MAIN.C:105-117`). `pixels[y][0]` is `image.pixels()[y*width]`
        // — a decoded `Image` is a `Surface`, whose rows are its width.
        let pixels = image.pixels();
        let inked = |x: usize| (0..rows).any(|y| pixels[y * width + x] != 0);

        let mut glyphs = Vec::new();
        let mut cells = Vec::new();
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
            cells.push((at, (x - start) as u16));
        }

        Ok(Font {
            glyphs,
            cells,
            cell,
            first,
        })
    }

    /// The cell for `ch` — the cell at index `ch as u32 - first as u32`: its
    /// pixels, `cell().1` rows of the glyph's own width, row-major, in the
    /// strip's own 2-bit values. `None` for a character before `first`, and
    /// for one past the strip's last cell.
    ///
    /// **The index is the character's place in the part's order, not its
    /// ASCII code.** The orders are the parts' own and are not contiguous —
    /// `ENDSCRL/MAIN.C:13` is `ABCDEFGHIJKLMNOPQRSTUVWX`, then `a`..`z`, then
    /// `0123456789`, then punctuation — so `glyph('Y')` is the strip's 25th
    /// cell (`ENDSCRL` has no `Y`; its 25th cell is `a`), and a caller that
    /// wants a character rather than a cell has to know its part's order.
    /// What the strip *does* guarantee is that the cells are in that order,
    /// one per character, starting at `first`.
    ///
    /// The cell's width is the run the scan found, `len() / cell().1`, and it
    /// is `3..25` for `ENDSCRL/FONA.UH`'s `!` and `W`: the widths are the
    /// strip's, and the blank is not here at all — the routine's blank cell
    /// is the strip's last cell, whose columns hold no ink for the scan to
    /// find (`ENDSCRL/MAIN.C:123-124` gives it a position and a width of 16,
    /// which is the part's business and not a glyph's).
    pub fn glyph(&self, ch: char) -> Option<&[u8]> {
        let index = (ch as u32).checked_sub(self.first as u32)? as usize;
        let &(at, width) = self.cells.get(index)?;
        let len = width as usize * self.cell.1 as usize;
        Some(&self.glyphs[at as usize..at as usize + len])
    }

    /// The cell this strip was read with — the part's routine's geometry, as
    /// [`Font::from_image`] documents it.
    pub fn cell(&self) -> (u16, u16) {
        self.cell
    }
}

/// The geometry and nothing else, for the same reason [`Image`]'s is: a
/// `Result<Font, AssetError>` in a test's `unwrap_err` has to drop the font,
/// and a strip's pixels do not belong in a failure's message.
impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("cell", &self.cell)
            .field("first", &self.first)
            .field("cells", &self.cells.len())
            .finish_non_exhaustive()
    }
}
