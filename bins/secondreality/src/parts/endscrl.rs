//! ENDSCRL — the greetings scroller: a ring of text, and the beam walking it.
//!
//! `ENDSCRL/MAIN.C` (125 lines) over `ENDSCRL/ASMYT.ASM` (75) — `waitvr`,
//! `setstart` and `setrgbpalette`, of which the part uses the last two. Its
//! acts, in order:
//!
//! 1. `:19-21` — `dis_partstart()` and one vertical blank.
//! 2. `:22-26` — mode 0eh (640×350, 16 colours) and then **CRTC 9 = 0**, which
//!    is the 400-line display the reference's 640×400 frames are.
//! 3. `:27-41` — fifteen `setrgbpalette` calls: entry 1 dark, 2 mid, 3-15
//!    light. Entry 0 is left alone, which is what the inherited DAC is for.
//! 4. `:43` — `init()`: the greetings text into a 64,000-byte buffer (`:101`)
//!    and the font strip's metric scan (`:103-124`).
//! 5. `:45-49` — `while(!dis_exit()) { if(dis_waitb()==1) dis_waitb();
//!    do_scroll(); }`: a line every two vertical blanks, for ever.
//!
//! `do_scroll` (`:60-96`) renders one line of the strip into four bit-planes
//! (`:75-92`) and moves the display start one line up the ring (`:93-95`).
//!
//! # The tick map
//!
//! A tick is one `dis_waitb`, and tick 0 is the code before the first one —
//! CRED's parked tick is the same convention, and the shell numbers a part's
//! ticks from its `start`. So `main`'s five acts are ticks 0, 1, 2, 3, 5, 7, …:
//!
//! - tick 0 (`:19-21`): nothing drawn. The screen is still the part before
//!   this one, which is what the loader shows while an EXE loads.
//! - tick 1 (`:22-43`): the mode set, the palette and `init`.
//! - tick 2: the loop's **first** wait, returning 1, with the second one still
//!   ahead of it — nothing is drawn on this tick either.
//! - tick 3 (`:48`): the first `do_scroll`. Each iteration of the loop consumes
//!   two waits (`:47`), so the next line lands on tick 5: **one line every two
//!   ticks**, which is what the reference's own frames measure (each picture on
//!   screen at index K is also the picture at K+1, `reference-indices.md`
//!   §21.1).
//!
//! # The ring: 400 lines where the original has 401, and why that is the same
//! picture
//!
//! `do_scroll` writes the line at `80*yscrl` **and again** at
//! `80*(yscrl+401)`, and steps `yscrl = (yscrl+1) % 401` (`:89-95`). One line
//! is 80 planar bytes, and a byte of a 16-colour plane is eight pixels, so
//! `80 * 8 = 640` pixels a line and the original's ring is `401 * 80 = 32,080`
//! bytes = 256,640 pixels, written twice. The second copy exists because the
//! display's address counter does not wrap at the ring's end: the start address
//! is set to a whole number of lines ahead of the write pointer, and the four
//! hundred lines the beam reads from there run off the end of the 401-line ring
//! and onto the copy.
//!
//! The port's beam **does** wrap: `View::source_index` is modulo the surface's
//! length (`view.rs`), and the port's surface is exactly 400 lines of 640
//! pixels = 256,000. So the port's ring is 400 lines, each line is written
//! once, and the wrap stands in for the second copy. The two pictures are not
//! merely similar — they are the same lines in the same rows:
//!
//! ```text
//! original, iteration t:  written line            t mod 401
//!                         start (post-increment) yscrl_post + 1 lines
//!                         displayed rows  j = 0..399 show  t - 399 + j
//! port,     iteration t:  written line            t mod 400
//!                         origin         (yscrl + 1) lines, yscrl = t mod 400
//!                         displayed rows  j = 0..399 show  t - 399 + j
//! ```
//!
//! Both put the line just written at the **bottom** row of the screen and the
//! one written 399 lines ago at the top, and both are the same line for every
//! `j`: the original's row `j` reads absolute line `((t+1) mod 401) + 1 + j`,
//! which is ring line `(t + 2 + j) mod 401` — most recently written at
//! iteration `t - 399 + j` (the copy at `+401` is what makes `j = 399` the
//! newest line rather than one that does not exist yet). The port's row `j`
//! reads ring line `(t + 1 + j) mod 400`, written at the same iteration.
//! [`tests::the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows`]
//! is that equality, checked row by row at every iteration of a turn.
//!
//! Three consequences, and
//! `the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows`
//! checks them as bytes over a whole turn:
//!
//! 1. each line is written **once** — no second copy, because the beam wraps;
//! 2. `yscrl` runs `0..400` where the original's runs `0..401`;
//! 3. the display origin is `(yscrl + 1) * 640` — 640 on the first line and
//!    **256,000** on the last, which is 124,930 past the largest origin the
//!    `u16` pair `start_addr + x_offset` could reach (`2 * 65,535 = 131,070`)
//!    and the reason Task 3 widened `start_addr` to `u32` (D16).
//!
//! # The line renderer, and the two units it mixes
//!
//! `:77-88` walks the line one pixel at a time, `mtau[x & 7]` selecting the
//! bit of the byte `x / 8` (`:55`, most significant bit first), and writes the
//! strip's four low bits into four planes: bit 0 into the byte written with
//! map mask 1, bit 3 into the one written with mask 8 (`:80-87`). The four
//! writes land on the *same* byte address in four planes, so the pixel they
//! spell is the strip's value 0..15 and the port can write it flat.
//!
//! The part reads the strip as `assets/ENDSCRL/FONA.UH` and the text as
//! `assets/MAIN/DATA/ENDSCROL.TXT`: the text the demo shipped
//! (`MAIN/PACKFINA.INC:51` packs exactly those 6,141 bytes), not the 1,007-byte
//! test text that sits beside the source.
//!
//! # What the reference is, and why its diff is not zero
//!
//! This part is a translation of the tree's source and reads the tree's assets;
//! the reference is the **released build**, and it draws this part differently —
//! a 25-row text pitch where `MAIN.C:6` says `FONAY 30`, a raster that differs
//! from `FONA.UH`'s in 53 pixels of the header band, and a text buffer with no
//! carriage returns — plus the chain's own 82-frame offset, measured in
//! `the_references_own_header_pins_the_chains_offset`. `docs/fidelity/endscrl.md`
//! §6 has the arithmetic of both and `docs/fidelity/APPROXIMATIONS.md` A11 is
//! the register entry.

use std::path::Path;

use sr_assets::font::Font;
use sr_assets::load_path;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "endscrl";

// ------------------------------------------------------------- the screen --

/// `MAIN.C:22-26`: mode 0eh is 640 pixels wide and 16 colours, and CRTC 9 = 0
/// is the 400-line display rather than the mode's own 350.
const VIEW_WIDTH: u16 = 640;
const VIEW_HEIGHT: u16 = 400;

/// `MAIN.C:89-92`: 80 bytes a line, one byte a plane, one byte eight pixels.
const LINE_BYTES: usize = 80;
const LINE_LEN: usize = LINE_BYTES * 8;

/// The port's ring, in lines: 400 × 640 = 256,000 = the whole surface, and the
/// wrap the original's second copy stands in for (module docs).
const RING_LINES: usize = 400;

// ------------------------------------------------------------ the palette --

/// `MAIN.C:27-41`: `setrgbpalette(1,20,20,20)`, `(2,40,40,40)`, then `(3..15,
/// 60,60,60)` — fifteen calls, and entry 0 is not one of them.
const PALETTE_FIRST: u8 = 1;
const PALETTE_LAST: u8 = 15;
const PALETTE_GREYS: [u8; 15] = [20, 40, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60];

// -------------------------------------------------------------- the strip --

/// `MAIN.C:12`: `char far font[31][1500]`, fifteen hundred columns of a 30-row
/// strip. The `.UH` beside the source is that raster plus its palette
/// (`ENDSCRL/ASMYT.ASM:73`'s `INCLUDE 'fona.inc'` is the payload alone).
const FONT_PATH: &str = "assets/ENDSCRL/FONA.UH";
/// `MAIN.C:12`: `char far font[31][1500]` — the raster's width, which is the
/// bound the scan walks (`:105`, `:112`).
const FONT_WIDTH: usize = 1500;
/// `MAIN.C:6`: `#define FONAY 30` — the rows the scan and the renderer read.
const FONT_ROWS: usize = 30;
/// The cell `sr_assets::font::Font` is built with: `MAIN.C:123`'s
/// `fonap[32]=1500-20` is the blank in the strip's *last* 20 columns, so the
/// strip is a whole number of 20-column cells (75) and the blank is the last.
const FONT_CELL: (u16, u16) = (20, FONT_ROWS as u16);
/// The first cell's character: every order opens `ABCDEFGHIJKLMNOPQRSTUVWX`.
const FIRST_CELL: char = 'A';
/// `MAIN.C:13`'s `fonaorder` — 74 characters, one per ink run the scan finds.
/// The two `\x8f` bytes are the source's own (`MAIN.C:13` spells them as two
/// bytes no code page here names); the shipped text uses neither.
const FONT_ORDER: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='";
/// `MAIN.C:123-124`: `fonap[32] = 1500-20`, `fonaw[32] = 16`. The strip's last
/// 20 columns hold no ink at all (the last inked column is 1,008), so the blank
/// draws nothing — but it *advances the pen* sixteen columns, which is why it
/// is a width and not a skip.
const BLANK: u8 = b' ';
const BLANK_WIDTH: usize = 16;
/// `[NO_CELL]` is a character with no glyph: one the order does not name, the
/// blank, or a run the scan did not find. The original's `fonaw` is 0 for all
/// three, so they advance the pen two pixels and draw nothing.
const NO_CELL: u8 = u8::MAX;

// --------------------------------------------------------------- the text --

/// The shipped greetings text: `MAIN.C:101` opens `"endscrol.txt"`, and the
/// shell's `int 21h` hook served the packed file — 6,141 bytes,
/// `MAIN/PACKFINA.INC:51` — which is `MAIN/DATA/ENDSCROL.TXT`. The 1,007-byte
/// `ENDSCRL/ENDSCROL.TXT` beside the source is the developer's own test text
/// ("Lerto has coded this hilariously fantastic…") and is not what the demo
/// showed. `docs/fidelity/asset-inventory.md` §E.9 measures both.
const TEXT_PATH: &str = "assets/MAIN/DATA/ENDSCROL.TXT";
/// `MAIN.C:52`'s `char far text[64000]` and `:101`'s `read(a,text,60000)`.
const TEXT_BUFFER: usize = 64_000;
const TEXT_READ_MAX: usize = 60_000;
/// `MAIN.C:68`'s `*tptr != '\n'`: the text is line-delimited, and the newline
/// itself is copied into `textline` and never drawn (`:73`).
const LINE_END: u8 = b'\n';
/// `MAIN.C:73`: `tstart=(639-tstart)/2` — centred on the 640-pixel line.
const TEXT_CENTRE: i32 = 639;
/// `MAIN.C:55`: `int mtau[8]={128,64,32,16,8,4,2,1}` — `x & 7` picks bit
/// `7 - (x & 7)`, so the leftmost pixel of a byte is its high bit.
const MTAU: [u8; 8] = [128, 64, 32, 16, 8, 4, 2, 1];
/// `MAIN.C:58`'s `scanbuf[4][80]`: four planes, one line each.
const PLANES: usize = 4;

// --------------------------------------------------------------- the loop --

/// The tick the mode set lands on, and the tick of the first `do_scroll` — see
/// the module docs' tick map. The two waits `:47` spends per iteration put
/// every later line two ticks after the last.
const MODE_TICK: u32 = 1;
const FIRST_SCROLL_TICK: u32 = 3;

// -------------------------------------------------------------- the strip --

/// The font strip and the two tables `MAIN.C`'s scan fills.
struct Strip {
    /// The scan's cells, as `sr_assets::font::Font` hands them out.
    font: Font,
    /// `fonaw[256]` (`MAIN.C:15`): each character's ink width, or 0 for one the
    /// order does not name. The blank's own entry is `:124`'s 16.
    fonaw: [u8; 256],
    /// `fonap[ch]`'s stand-in: the glyph's index among the scan's cells, or
    /// [`NO_CELL`]. The scan's cells *are* its ink runs, so this is the same
    /// character-to-column mapping `fonap` is, in the form `Font` indexes by.
    cell: [u8; 256],
}

impl Strip {
    /// `MAIN.C:103-122` — the scan, one character of `fonaorder` per ink run.
    ///
    /// The routine walks `x` to 1,500, skips blank columns, takes the run of
    /// inked ones and hands it to the next character. `Font::from_image` runs
    /// the same loop over the strip's bytes (`font.rs`), and its cells are the
    /// runs it found, so the port's widths are the source's `fonaw` rather than
    /// a slice of the raster at a stated pitch — the distinction `font.rs`
    /// documents, and the reason `CREDITS`' strip is read differently there.
    fn scan(font: Font) -> Strip {
        let mut fonaw = [0u8; 256];
        let mut cell = [NO_CELL; 256];
        for (index, character) in FONT_ORDER.iter().enumerate() {
            let Some(glyph) = font.glyph(cell_char(index)) else {
                continue;
            };
            let width = glyph.len() / FONT_ROWS;
            fonaw[usize::from(*character)] = width as u8;
            cell[usize::from(*character)] = index as u8;
        }
        fonaw[usize::from(BLANK)] = BLANK_WIDTH as u8;
        Strip { font, fonaw, cell }
    }

    /// The glyph for `character`, `FONT_ROWS` rows of `fonaw[character]`
    /// columns, or `None` for a character that has none.
    fn glyph(&self, character: u8) -> Option<&[u8]> {
        let cell = self.cell[usize::from(character)];
        if cell == NO_CELL {
            return None;
        }
        self.font.glyph(cell_char(usize::from(cell)))
    }
}

/// `MAIN.C:119-120`'s mapping from an order position to a cell, in the form
/// [`Font::glyph`] takes: the first cell's character plus the index.
fn cell_char(index: usize) -> char {
    char::from_u32(FIRST_CELL as u32 + index as u32).expect("a cell index is a character")
}

// ---------------------------------------------------------------- the part --

/// ENDSCRL: the ring, the pen and the beam.
pub struct Endscrl {
    /// The strip, once `init` has read it.
    strip: Option<Strip>,
    /// `MAIN.C:52`'s `text[64000]`: the file's bytes at the front, the rest
    /// zero, exactly as `:101`'s `read` leaves it.
    text: Vec<u8>,
    /// `MAIN.C:53`'s `tptr`.
    tptr: usize,
    /// `MAIN.C:57`'s `textline` and `:54`'s `chars`: the line being drawn,
    /// re-read from the text whenever `line` comes round to 0. The source's
    /// array is 100 bytes and written without a bound; the shipped text's
    /// longest line is 40 characters (`the_shipped_text_fits_the_row_…`), so the
    /// port grows this instead of overflowing it.
    textline: Vec<u8>,
    chars: usize,
    /// `MAIN.C:54`'s `tstart`: the pen's first column on the line.
    tstart: i32,
    /// `MAIN.C:62`'s `yscrl`: the ring line the next `do_scroll` writes.
    yscrl: usize,
    /// `MAIN.C:63`'s `line`: the strip row it is drawn from.
    line: usize,
    /// The part's own tick, which is the shell's count for this part.
    tick: u32,
    /// Where the two assets are read from. A unit test's working directory is
    /// this crate's, not the repository root, so it reads `../../assets/…`.
    font_path: &'static str,
    text_path: &'static str,
}

impl Endscrl {
    /// A part with nothing loaded: `init` reads the strip and the text.
    pub fn new() -> Endscrl {
        Endscrl {
            strip: None,
            text: Vec::new(),
            tptr: 0,
            textline: Vec::new(),
            chars: 0,
            tstart: 0,
            yscrl: 0,
            line: 0,
            tick: 0,
            font_path: FONT_PATH,
            text_path: TEXT_PATH,
        }
    }

    /// [`new`](Endscrl::new) reading both assets from elsewhere.
    #[cfg(test)]
    fn at(font: &'static str, text: &'static str) -> Endscrl {
        Endscrl { font_path: font, text_path: text, ..Endscrl::new() }
    }

    /// `MAIN.C:22-43` — the mode, the palette, `init`, all of it one tick.
    fn enter_mode(&self, ctx: &mut PartCtx) {
        // `:22-26`: mode 0eh, then CRTC 9 = 0. The mode set is also what clears
        // the video memory (`int10_modes.cpp`'s `finish_set_mode`, `:813` and
        // `:859`), and the memory it clears is CRED's: this is the tick the
        // black 640x400 screen appears on, and why the ring's unwritten lines
        // are black rather than the previous part's picture.
        ctx.surface.clear(0);
        // `:27-41`: fifteen `setrgbpalette` calls. Entry 0 and 16-255 are left
        // where the previous part left them — the reference's own frames show
        // entry 0 black, which is the repaired ramp CRED's last screen ends on.
        for entry in PALETTE_FIRST..=PALETTE_LAST {
            let grey = PALETTE_GREYS[usize::from(entry - PALETTE_FIRST)];
            ctx.set_palette_entry(entry, [grey, grey, grey]);
        }
        Endscrl::set_display(ctx, 0);
    }

    /// The display `MAIN.C:22-26` leaves and `:95` moves: 640 pixels a row, 400
    /// rows, no doubling, no split, and the beam's origin.
    fn set_display(ctx: &mut PartCtx, origin: u32) {
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        // The display's line stride *is* the ring's line — 640 pixels, which is
        // also the surface's own row stride. Written out because the part
        // addresses the ring itself (Part B's second convention): a stride left
        // to resolve is one that could resolve to a different layout.
        ctx.view.y_line_offset = Some(LINE_LEN as u16);
        ctx.view.double_scan = false; // CRTC 9 = 0
        ctx.view.pan = 0;
        ctx.view.split = None;
        ctx.view.set_origin(origin);
    }

    /// `MAIN.C:66-74` — the next line of the text, and where it starts.
    ///
    /// The scan is the source's: characters up to the newline, each adding
    /// `fonaw[ch] + 2` to the line's width, the newline itself copied into
    /// `textline` and stepped over (`:73`), and the whole line centred by
    /// `(639 - width) / 2` — integers, so an odd width starts half a pixel left
    /// of centre, exactly as the original's truncating divide does.
    fn read_line(&mut self) {
        let mut width = 0i32;
        self.textline.clear();
        // `for(a=0,tstart=0,chars=0; *tptr != '\n'; a++,chars++)`. The source's
        // buffer has no terminator if the text runs out; the port stops at the
        // buffer's end, which nothing in this tree reaches — the shipped text is
        // 542 lines and the reference's own longest run is 184 lines in
        // (`the_shipped_text_fits_the_row_and_the_textline_buffer` for the 542,
        // `the_references_own_header_pins_the_chains_offset` for the 184).
        while self.tptr < TEXT_BUFFER && self.text[self.tptr] != LINE_END {
            let character = self.text[self.tptr];
            width += i32::from(self.strip.as_ref().expect("init read the strip").fonaw
                [usize::from(character)])
                + 2;
            self.textline.push(character);
            self.tptr += 1;
        }
        self.tptr += 1; // `textline[a]=*tptr++`: the newline, and past it
        self.chars = self.textline.len();
        self.tstart = (TEXT_CENTRE - width) / 2;
    }

    /// `MAIN.C:75-88` — the line, as the 640 pixels its four planes spell.
    ///
    /// Literal, unit for unit: `scanbuf` is four rows of 80 bytes (`:58`), the
    /// pen walks the line one pixel at a time with `mtau[x & 7]` selecting the
    /// bit of the byte `x / 8` (`:55`, most significant bit first), and the
    /// strip's four low bits go one per plane (`:80-87`). Four byte addresses
    /// at one offset are four pixels' worth — one pixel each, plane `p` being
    /// the colour's bit `p` — so the port folds them into the flat value its
    /// surface holds, and `do_scroll` copies that once.
    fn render_line(&mut self) -> [u8; LINE_LEN] {
        let mut scanbuf = [0u8; PLANES * LINE_BYTES]; // `:75`'s `memset(…,80*4)`
        let mut x = self.tstart;
        for index in 0..self.chars {
            let character = self.textline[index];
            let strip = self.strip.as_ref().expect("init read the strip");
            let width = usize::from(strip.fonaw[usize::from(character)]);
            let glyph = strip.glyph(character);
            for column in 0..width {
                let value = glyph.map_or(0, |glyph| glyph[self.line * width + column]);
                // `scanbuf[plane][x/8] ^= mtau[x & 7]`. The shipped text keeps
                // `x` inside the row (`the_shipped_text_fits_the_row_…`); the
                // original writes outside `scanbuf` if it does not, which is
                // undefined there, so the port drops those pixels instead of
                // reading a neighbouring static.
                if (0..LINE_LEN as i32).contains(&x) {
                    let bit = MTAU[(x & 7) as usize];
                    for plane in 0..PLANES {
                        if value & (1 << plane) != 0 {
                            scanbuf[plane * LINE_BYTES + (x / 8) as usize] ^= bit;
                        }
                    }
                }
                x += 1;
            }
            x += 2; // `for(a=…;a<chars;a++,x+=2)`: the two-column gap
        }

        // The memcpy of `:89-92`, one line of it, resolved: the four planes are
        // the pixel's four bits.
        let mut row = [0u8; LINE_LEN];
        for (pixel, out) in row.iter_mut().enumerate() {
            let mut value = 0u8;
            for plane in 0..PLANES {
                if scanbuf[plane * LINE_BYTES + pixel / 8] & MTAU[pixel & 7] != 0 {
                    value |= 1 << plane;
                }
            }
            *out = value;
        }
        row
    }

    /// `MAIN.C:60-96` — one line of the text, and the display start moved on.
    ///
    /// The source's statics are this struct's fields: `yscrl` (`:62`) is the
    /// ring line, `line` (`:63`) the strip row, `tptr`/`textline`/`chars`/
    /// `tstart` (`:53-54`, `:57`) the line being drawn.
    fn do_scroll(&mut self, ctx: &mut PartCtx) {
        if self.line == 0 {
            self.read_line();
        }
        let row = self.render_line();
        // `:89-92`: `memcpy(MK_FP(0x0a000,80*yscrl), …)` per plane — the line,
        // once. The original writes it a second time at `80*(yscrl+401)`; the
        // port's beam wraps instead (module docs).
        let at = self.yscrl * LINE_LEN;
        ctx.surface.pixels_mut()[at..at + LINE_LEN].copy_from_slice(&row);

        // `:95`'s `setstart(yscrl*80 + 80*1)`, with the **post**-increment
        // `yscrl`: one line after the one just written, in the original's
        // 401-line bytes. Read as the port's ring, that line is
        // `(yscrl + 1) * 640` for the `yscrl` before the step — the same
        // picture (module docs), and 256,000 on the last line of a turn.
        let origin = (self.yscrl + 1) * LINE_LEN;
        self.yscrl = (self.yscrl + 1) % RING_LINES;
        self.line = (self.line + 1) % FONT_ROWS;
        Endscrl::set_display(ctx, origin as u32);
    }
}

impl Part for Endscrl {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:97-125`: the text into the buffer, and the strip's metrics.
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        let image = load_path(Path::new(self.font_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.font_path),
        })?;
        if image.width() as usize != FONT_WIDTH {
            // `:105`'s scan is bounded by the raster's own 1,500 columns; a
            // strip of another width is a different asset, not this one read
            // differently.
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} is {} columns; this part's strip is {FONT_WIDTH}",
                    self.font_path,
                    image.width()
                ),
            });
        }
        let font = Font::from_image(image, FONT_CELL, FIRST_CELL).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.font_path),
        })?;
        self.strip = Some(Strip::scan(font));

        let file = std::fs::read(self.text_path).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.text_path),
        })?;
        // `read(a,text,60000)`: at most 60,000 bytes into a 64,000-byte buffer,
        // whose tail stays zero.
        let read = file.len().min(TEXT_READ_MAX);
        self.text = vec![0; TEXT_BUFFER];
        self.text[..read].copy_from_slice(&file[..read]);
        Ok(())
    }

    /// One tick of `MAIN.C:19-49`. See the module docs' tick map.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match self.tick {
            // `:19-21`: the part's own blank. The screen is still CRED's.
            0 => {}
            MODE_TICK => self.enter_mode(ctx),
            tick if tick >= FIRST_SCROLL_TICK && (tick - FIRST_SCROLL_TICK) % 2 == 0 => {
                self.do_scroll(ctx)
            }
            // The loop's second wait (`:47`): a tick with nothing drawn on it,
            // which is what makes the scroll step two frames wide.
            _ => {}
        }
        self.tick += 1;
        // `:45-49`: `while(!dis_exit())`, and `dis_exit` is only ever set by a
        // key (`DIS/DISINT.ASM`'s `checkkeys`). The shell is what sees it —
        // the part itself never ends.
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::view::View;
    use sr_engine::{Palette, Surface};

    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// The paths a unit test reads: this crate's own directory, two levels
    /// below the repository root, so the part's own relative paths do not
    /// resolve there.
    const TEST_FONT: &str = "../../assets/ENDSCRL/FONA.UH";
    const TEST_TEXT: &str = "../../assets/MAIN/DATA/ENDSCROL.TXT";
    /// The file the part must *not* read (`reference-indices.md` §21).
    const DEVELOPERS_TEXT: &str = "../../assets/ENDSCRL/ENDSCROL.TXT";

    /// The engine's own pieces held together, so a `PartCtx` can be built for
    /// one call at a time — the same disjoint borrows `Engine::tick` makes.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
    }

    impl Rig {
        fn new() -> Rig {
            Rig {
                surface: Surface::new(SURFACE_WIDTH, SURFACE_HEIGHT),
                view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
                palette: Palette::new(),
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
            }
        }

        fn ctx(&mut self, frame: u32) -> PartCtx<'_> {
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                // no player: this rig draws a band, and none of these parts asks for a cue
                None,
                frame,
                0,
                1,
                true,
                false,
            )
        }
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Endscrl) {
        let mut rig = Rig::new();
        let mut part = Endscrl::at(TEST_FONT, TEST_TEXT);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Runs the part up to (and not including) `tick`, numbering each call by
    /// the part's own tick count — which is what the shell does.
    fn run_to(rig: &mut Rig, part: &mut Endscrl, tick: u32) {
        while part.tick < tick {
            let at = part.tick;
            assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Continuing, "tick {at}");
        }
    }

    /// Runs the part for `ticks` further ticks.
    fn run_ticks(rig: &mut Rig, part: &mut Endscrl, ticks: u32) {
        let until = part.tick + ticks;
        run_to(rig, part, until);
    }

    /// The port's displayed frame as RGB, one pixel per displayed pixel, through
    /// the live palette and the engine's own 6→8-bit widening — the same bytes
    /// the binary's `--dump-frames` writes (`headless::rgb8`).
    ///
    /// The origin and the stride are whole lines, so a displayed row's 640
    /// pixels are contiguous (`the_four_hundred_line_ring_shows_…` asserts it).
    fn rendered_frame(rig: &Rig) -> Vec<u8> {
        let mut out = vec![0u8; VIEW_WIDTH as usize * VIEW_HEIGHT as usize * 3];
        for y in 0..VIEW_HEIGHT as usize {
            let at = rig.view.source_index(&rig.surface, 0, y as u16);
            for (x, pixel) in rig.surface.pixels()[at..][..LINE_LEN].iter().enumerate() {
                let entry = rig.palette.entry(*pixel);
                let at = (y * LINE_LEN + x) * 3;
                out[at] = sr_engine::dac_expand(entry[0]);
                out[at + 1] = sr_engine::dac_expand(entry[1]);
                out[at + 2] = sr_engine::dac_expand(entry[2]);
            }
        }
        out
    }

    /// How many pixels the port's frame differs from `reference` in, over the
    /// rows `top..=bottom`, with the reference's pixels read `shift` columns to
    /// the right of the port's — the ship's one-column difference, so that the
    /// count is of the *raster* rather than of a shift.
    fn differing_pixels(rig: &Rig, reference: &[u8], top: usize, bottom: usize, shift: usize) -> usize {
        let ours = rendered_frame(rig);
        let mut differing = 0usize;
        for y in top..=bottom {
            for x in 0..LINE_LEN - shift {
                let at = (y * LINE_LEN + x) * 3;
                if ours[at..at + 3] != reference[at + shift * 3..at + shift * 3 + 3] {
                    differing += 1;
                }
            }
        }
        differing
    }

    /// The reference's own frame, read from the tree it is committed in.
    fn reference(index: u32) -> Vec<u8> {
        std::fs::read(format!("../../reference/parts/endscrl/frame_{index:06}.rgb")).unwrap()
    }

    /// How many rows of the strip hold ink: `0..=17`, which
    /// `the_strip_is_the_one_the_part_scans` asserts. It is what makes a
    /// 25-row gap a line gap rather than one line's fragments.
    const STRIP_INK_ROWS: usize = 18;

    /// The reference's own CRED→ENDSCRL bracket, measured in the captures. The
    /// sweep's log shows 8840 still CRED's screen — so the last CRED frame, and
    /// this part's tick 0, is at 8841 or later — and refuses 8940 as `640x400`,
    /// so the geometry change is at 8940 or before; the port's change is its
    /// tick 2 (`tests/endscrl.rs` measures it at 8837 for a tick 0 of 8835), so
    /// the start lies in **`[8841, 8938]`** — `reference-indices.md` §20/§21,
    /// `docs/fidelity/cred.md` §7.
    const REFERENCE_FIRST: u32 = 8841;
    const REFERENCE_LAST: u32 = 8938;

    /// Where the port's chain hands this part its tick 0 — measured through the
    /// binary in `tests/endscrl.rs` from the dump's own `part` fields.
    const PORT_FIRST: u32 = 8835;

    #[test]
    fn the_strip_is_the_one_the_part_scans() {
        // `MAIN.C:12`'s raster, as the part reads it: 1,500 columns of 30 rows,
        // and one ink run per character of `:13`'s order. `font.rs` refuses
        // `CREDITS`' strip because its stated cell does not divide it; this one
        // it accepts, and its cells are the same runs `MAIN.C:103-122` finds.
        let (_, part) = started();
        let strip = part.strip.as_ref().expect("init read the strip");
        assert_eq!(strip.font.cell(), FONT_CELL);
        let widths: Vec<usize> =
            FONT_ORDER.iter().map(|ch| usize::from(strip.fonaw[usize::from(*ch)])).collect();
        assert_eq!(widths.len(), 74, "one run per character of the order");
        assert!(widths.iter().all(|width| *width > 0), "a character with no run: {widths:?}");
        assert_eq!(widths.iter().copied().min(), Some(3), "the narrowest run");
        assert_eq!(widths.iter().copied().max(), Some(25), "the widest");
        assert_eq!(strip.fonaw[usize::from(b'A')], 20);
        assert_eq!(strip.fonaw[usize::from(b'W')], 25);
        // The blank: `:123-124`, sixteen columns of the strip that hold no ink.
        assert_eq!(strip.fonaw[usize::from(BLANK)], 16);
        assert!(strip.glyph(BLANK).is_none(), "the blank is not a cell of the scan");
        // A character the order does not name has no width at all (the static
        // array's 0), which is what the text's `\r` and its one `\x84` need.
        for absent in [b'\r', 0x84, b'Y', b'Z', 0x00] {
            assert_eq!(strip.fonaw[usize::from(absent)], 0, "{absent}");
            assert!(strip.glyph(absent).is_none(), "{absent}");
        }
        // The strip's ink lives in rows 0-17: which is what makes the
        // reference's 25-row gaps *line* gaps — a line's ink is 18 rows tall, so
        // two bands further apart than that are two lines and not one line's
        // fragments.
        let inked_rows: Vec<usize> = (0..FONT_ROWS)
            .filter(|row| {
                (0..FONT_ORDER.len()).any(|index| {
                    let glyph = strip.font.glyph(cell_char(index)).unwrap();
                    let width = glyph.len() / FONT_ROWS;
                    (0..width).any(|column| glyph[row * width + column] != 0)
                })
            })
            .collect();
        assert_eq!(inked_rows, (0..STRIP_INK_ROWS).collect::<Vec<_>>(), "the strip's ink rows");

        // The raster's own columns, from the image rather than from the scan:
        // 932 inked columns, the first at 0 and the last at **1,008** — which is
        // what makes `:123`'s `fonap[32]=1500-20` a blank cell: the strip's last
        // 491 columns hold no ink at all.
        let image = load_path(Path::new(TEST_FONT)).unwrap();
        let inked_columns: Vec<usize> = (0..FONT_WIDTH)
            .filter(|x| (0..FONT_ROWS).any(|y| image.pixels()[y * FONT_WIDTH + x] != 0))
            .collect();
        assert_eq!(inked_columns.len(), 932, "the strip's inked columns");
        assert_eq!(inked_columns.first(), Some(&0));
        assert_eq!(inked_columns.last(), Some(&1_008), "the strip's last inked column");

        // And the strip is the tree's own art, not a conversion of it that lost
        // pixels: `ENDSCRL/FONA.LBM` decodes to the same raster value for value
        // (`docs/fidelity/asset-inventory.md` §E.8 reads the two as source and
        // converted form).
        let lbm = load_path(Path::new("../../assets/ENDSCRL/FONA.LBM")).unwrap();
        assert_eq!((lbm.width(), lbm.height()), (FONT_WIDTH as u16, FONT_ROWS as u16));
        assert_eq!(lbm.pixels(), image.pixels(), "FONA.LBM and FONA.UH are not the same raster");
        // The strip's own values: a 2-bit mask per pixel, which is exactly the
        // four plane bits `:80-87` tests.
        for index in 0..74 {
            let glyph = strip.font.glyph(cell_char(index)).unwrap();
            assert!(glyph.iter().all(|value| *value < 4), "cell {index} is not a 2-bit mask");
        }
    }

    #[test]
    fn the_text_is_the_shipped_one_and_not_the_developers() {
        // The two files, and the one the part reads. `MAIN.C:101` opens
        // `"endscrol.txt"` with no path, and the shell's `int 21h` hook served
        // the packed file: `MAIN/PACKFINA.INC:51` gives it as 0x17FD = 6,141
        // bytes, which is `MAIN/DATA/ENDSCROL.TXT` and not the 1,007-byte file
        // that sits in the source directory.
        let shipped = std::fs::read(TEST_TEXT).unwrap();
        let developers = std::fs::read(DEVELOPERS_TEXT).unwrap();
        assert_eq!(shipped.len(), 6_141, "the shipped text");
        assert_eq!(developers.len(), 1_007, "the developer's own test text");
        assert_ne!(shipped, developers);

        let (_, part) = started();
        assert_eq!(&part.text[..shipped.len()], &shipped[..], "the part read the other file");
        assert!(
            part.text[shipped.len()..].iter().all(|byte| *byte == 0),
            "`read` leaves the buffer's tail as it found it"
        );
        // And the first line it draws is the shipped one's: the developer's
        // text opens "Lerto has coded this hilariously fantastic".
        assert!(
            shipped.starts_with(b"Second Reality\r\n"),
            "the shipped text does not open with the copyright card"
        );
        assert!(
            developers.starts_with(b"Lerto has coded this"),
            "the developer's text does not open the way the record says"
        );
    }

    #[test]
    fn the_shipped_text_fits_the_row_and_the_textline_buffer() {
        // Two bounds the source's own arrays have and never checks: `textline`
        // is 100 bytes (`MAIN.C:57`) and `scanbuf` is 80 bytes a plane (`:58`),
        // both written without a bound. The shipped text stays inside them, so
        // the port's bounds checks are never reached — and they are checked
        // here rather than trusted.
        let shipped = std::fs::read(TEST_TEXT).unwrap();
        let (_, part) = started();
        let strip = part.strip.as_ref().unwrap();
        let mut lines = 0usize;
        for line in shipped.split(|byte| *byte == LINE_END) {
            // The last element of a text that ends in a newline is empty.
            if line.is_empty() {
                continue;
            }
            lines += 1;
            let width: i32 =
                line.iter().map(|ch| i32::from(strip.fonaw[usize::from(*ch)]) + 2).sum();
            let tstart = (TEXT_CENTRE - width) / 2;
            assert!(tstart >= 0, "line {lines}: {width} columns does not fit the row");
            assert!(tstart + width < LINE_LEN as i32, "line {lines}: runs past the row");
            assert!(line.len() <= 100, "line {lines}: {} characters", line.len());
        }
        assert_eq!(lines, 542, "the shipped text's lines");
        // The two numbers the row's own fit turns on, asserted rather than
        // described: the widest line is 579 columns, so its `tstart` is 30, and
        // the last column any line draws is 607 of 640.
        let widest = shipped
            .split(|byte| *byte == LINE_END)
            .map(|line| {
                line.iter().map(|ch| i32::from(strip.fonaw[usize::from(*ch)]) + 2).sum::<i32>()
            })
            .max()
            .unwrap();
        assert_eq!(widest, 579, "the widest line's width");
        assert_eq!((TEXT_CENTRE - widest) / 2, 30, "its first column");
        assert_eq!((TEXT_CENTRE - widest) / 2 + widest - 2, 607, "the last column drawn");
    }

    #[test]
    fn the_mode_set_is_the_four_hundred_line_screen_with_the_inherited_dac() {
        // `:22-43`: mode 0eh, CRTC 9 = 0, then the fifteen greys and `init`.
        // The mode set clears the video memory (`int10_modes.cpp`'s
        // `finish_set_mode`, `:813` and `:859`): the screen is CRED's until
        // this tick and black from it, which is what the reference's own frames
        // show — 252,312 of frame 9100's 256,000 pixels are (0,0,0) and the
        // part writes no background of its own.
        let (mut rig, mut part) = started();
        for pixel in rig.surface.pixels_mut() {
            *pixel = 5;
        }
        // Two entries the part must not touch, standing in for the DAC CRED
        // left behind: entry 0 (which the reference's frames show black) and
        // one above the part's fifteen.
        rig.palette.set_entry(0, [1, 2, 3]);
        rig.palette.set_entry(16, [4, 5, 6]);

        run_ticks(&mut rig, &mut part, MODE_TICK + 1);

        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "the mode set clears it");
        assert_eq!((rig.view.width, rig.view.height), (VIEW_WIDTH, VIEW_HEIGHT));
        assert_eq!(rig.view.y_line_offset, Some(LINE_LEN as u16), "the display's own stride");
        assert!(!rig.view.double_scan, "CRTC 9 = 0");
        assert_eq!(rig.view.start_addr, 0, "a mode set leaves the start address at 0");
        assert!(rig.view.split.is_none());
        for (index, grey) in PALETTE_GREYS.iter().enumerate() {
            let entry = PALETTE_FIRST + index as u8;
            assert_eq!(rig.palette.entry(entry), [*grey, *grey, *grey], "entry {entry}");
        }
        assert_eq!(rig.palette.entry(0), [1, 2, 3], "entry 0 is not the part's to write");
        assert_eq!(rig.palette.entry(16), [4, 5, 6], "nor is 16");
    }

    #[test]
    fn a_line_is_written_once_and_the_beam_wraps_instead_of_a_second_copy() {
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, FIRST_SCROLL_TICK);
        assert!(
            rig.surface.pixels().iter().all(|pixel| *pixel == 0),
            "ticks 0-2 draw nothing"
        );

        part.frame(&mut rig.ctx(FIRST_SCROLL_TICK));

        // `:89-92`: the line, 80 bytes a plane, at `80*yscrl` — and the original
        // writes it a second time at `80*(yscrl+401)`. One line is 640 pixels
        // and the port's surface is 400 of them, so that address is off the end
        // of the port's ring: the beam's wrap is the second copy.
        assert_eq!(part.yscrl, 1, "the write pointer steps");
        let inked = |row: &[u8]| row.iter().filter(|pixel| **pixel != 0).count();
        assert!(inked(&rig.surface.pixels()[..LINE_LEN]) > 0, "the line was not drawn");
        assert_eq!(inked(&rig.surface.pixels()[LINE_LEN..]), 0, "something outside the line moved");
        assert!(
            (RING_LINES + 1) * LINE_LEN > rig.surface.pixels().len(),
            "the original's second copy would be at ring line 401, past this surface"
        );

        // `:95`'s `setstart(yscrl*80 + 80)`, with the post-increment `yscrl`:
        // 160 planar bytes = 2 lines of the original's memory, whose second copy
        // makes the line just written the *bottom* row of the display. In the
        // port's units that is `(yscrl + 1) * 640` = 640, read modulo the
        // surface.
        assert_eq!(rig.view.start_addr, LINE_LEN as u32);
        assert_eq!(
            rig.view.source_index(&rig.surface, 0, VIEW_HEIGHT - 1),
            0,
            "the bottom row is the line just written"
        );
        assert_eq!(
            rig.view.source_index(&rig.surface, 0, 0),
            LINE_LEN,
            "and the top row is the oldest line of the ring"
        );
    }

    #[test]
    fn the_display_advances_one_line_every_two_ticks() {
        // `:47`'s `if(dis_waitb()==1) dis_waitb();` — the loop spends two waits
        // an iteration, so a line lands every second tick: 3, 5, 7, … and no
        // tick in between moves anything.
        let (mut rig, mut part) = started();
        let mut state = Vec::new();
        for at in 0..12u32 {
            part.frame(&mut rig.ctx(at));
            state.push((part.yscrl, rig.view.start_addr));
        }
        assert_eq!(
            &state[..3],
            &[(0, 0), (0, 0), (0, 0)],
            "nothing scrolls before the first `do_scroll`"
        );
        for (line, pair) in [(1usize, &state[3..5]), (2, &state[5..7]), (3, &state[7..9]), (4, &state[9..11])] {
            let origin = line as u32 * LINE_LEN as u32;
            assert_eq!(pair, &[(line, origin), (line, origin)], "the pair that shows line {line}");
        }
        assert_eq!(state[11], (5, 5 * LINE_LEN as u32));
    }

    #[test]
    fn the_ring_wraps_at_four_hundred_lines_not_four_hundred_and_one() {
        // `MAIN.C:93`: `yscrl=(yscrl+1)%401` — and `:89`'s second copy is what
        // makes 401 work there. The port's beam wraps at 400 lines: 400 × 640 =
        // 256,000 = the surface, so the write pointer and the origin the beam
        // reads come back to the state one turn ago, exactly. The arithmetic
        // that makes 401 impossible here: a 401st line's origin would be
        // `401 * 640 = 256,640`, past the end of everything the beam can read.
        let (mut rig, mut part) = started();

        run_ticks(&mut rig, &mut part, FIRST_SCROLL_TICK + 2); // the first line: ticks 3 and 4
        let first = (part.yscrl, rig.view.start_addr);
        assert_eq!(first, (1, LINE_LEN as u32));

        // 400 lines is 800 ticks; the 400th lands on tick 801.
        run_to(&mut rig, &mut part, FIRST_SCROLL_TICK + 2 * RING_LINES as u32);
        assert_eq!(part.yscrl, 0, "400 lines is a whole turn");
        assert_eq!(
            rig.view.start_addr,
            400 * LINE_LEN as u32,
            "the last line's origin is the largest there is — 256,000, the number Task 3's \
             widening is for — and the beam reads it as ring line 0"
        );
        assert_eq!(rig.view.source_index(&rig.surface, 0, 0), 0);
        assert_eq!(RING_LINES * LINE_LEN, rig.surface.pixels().len());
        // And the number the widening was for: the largest origin a `u16` pair
        // could sum to, against the one this ring reaches.
        assert_eq!(400 * LINE_LEN - 2 * usize::from(u16::MAX), 124_930);

        // One more line, and the state is the one after the first: the turn is
        // 400 lines, not 401. **The pixels are not the same** — the ring now
        // holds a different line of the greetings text, which is 542 lines long
        // — but the write pointer and the origin the beam reads are, and that
        // is what "the picture at 800 ticks" means here: a line every two ticks
        // is a 400-line turn, where the original's 401-line ring turns in 802.
        part.frame(&mut rig.ctx(FIRST_SCROLL_TICK + 2 * RING_LINES as u32));
        assert_eq!((part.yscrl, rig.view.start_addr), first);
    }

    #[test]
    fn the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows() {
        // The modelling claim itself, checked against the original's own
        // arithmetic rather than argued: a 401-line ring written **twice** — at
        // `80*yscrl` and `80*(yscrl+401)` — with the display start at
        // `yscrl*80 + 80`, against the port's 400-line ring and its origin.
        //
        // Both sides are driven by the port's own rendered lines, iteration by
        // iteration, and the 400 displayed rows are compared as bytes: the
        // original's row j is at absolute line `((t+1) % 401) + 1 + j` of its
        // 802-line memory, the port's at `(origin/640 + j) % 400`.
        let (mut rig, mut part) = started();
        let mut original = vec![0u8; 2 * (RING_LINES + 1) * LINE_LEN];
        let mut compared = 0usize;

        for at in 0..(FIRST_SCROLL_TICK + 2 * 500) {
            let before = part.yscrl;
            part.frame(&mut rig.ctx(at));
            if at < FIRST_SCROLL_TICK || at % 2 == 0 {
                continue;
            }
            let iteration = ((at - FIRST_SCROLL_TICK) / 2) as usize;
            let written = &rig.surface.pixels()[before * LINE_LEN..][..LINE_LEN];
            let original_row = iteration % (RING_LINES + 1);
            original[original_row * LINE_LEN..][..LINE_LEN].copy_from_slice(written);
            original[(original_row + RING_LINES + 1) * LINE_LEN..][..LINE_LEN]
                .copy_from_slice(written);

            let start = original_row + 2; // `setstart` with the post-increment `yscrl`
            for row in 0..VIEW_HEIGHT as usize {
                // The port's row starts are whole lines, so a row's 640 pixels
                // are contiguous: `source_index` of its first pixel is a
                // multiple of 640 and the last one is inside the surface.
                let at = rig.view.source_index(&rig.surface, 0, row as u16);
                assert_eq!(at % LINE_LEN, 0, "iteration {iteration}, row {row} is not a line");
                assert_eq!(
                    &rig.surface.pixels()[at..][..LINE_LEN],
                    &original[(start + row) * LINE_LEN..][..LINE_LEN],
                    "iteration {iteration}, displayed row {row}"
                );
            }
            compared += 1;
        }
        assert_eq!(compared, 500, "every iteration of a turn was compared");
    }

    /// The rows of `frame` that hold ink, as (first, last) pairs — one per run
    /// of consecutive rows, which is one band: a line of text, or a fragment of
    /// one where the shipped raster leaves a row empty inside a line.
    fn ink_bands(frame: &[u8]) -> Vec<(usize, usize)> {
        let mut bands: Vec<(usize, usize)> = Vec::new();
        for y in 0..VIEW_HEIGHT as usize {
            let row = &frame[y * LINE_LEN * 3..][..LINE_LEN * 3];
            if row.iter().any(|byte| *byte != 0) {
                match bands.last_mut() {
                    Some(last) if last.1 + 1 == y => last.1 = y,
                    _ => bands.push((y, y)),
                }
            }
        }
        bands
    }

    /// The bands that are whole lines of text: fragments closer together than
    /// one line is tall are one line's, merged (the shipped raster leaves a row
    /// empty inside a line where a glyph's dot stands alone, which splits it).
    fn text_lines(frame: &[u8]) -> Vec<(usize, usize)> {
        let mut lines: Vec<(usize, usize)> = Vec::new();
        for (top, bottom) in ink_bands(frame) {
            match lines.last_mut() {
                // A line's ink is 18 rows tall at most, so two bands whose span
                // is no taller than that cannot be two lines.
                Some(last) if bottom - last.0 < STRIP_INK_ROWS => last.1 = bottom,
                _ => lines.push((top, bottom)),
            }
        }
        lines
    }

    /// The text's lines, one per newline, the last empty when the file ends in
    /// one — `read_line`'s own unit.
    fn text_lines_of(text: &[u8]) -> Vec<&[u8]> {
        text.split(|byte| *byte == LINE_END).collect()
    }

    /// The ring rows one text line advances the display by, in the **shipped**
    /// build: `NN` for a `[NN` control line, 25 for anything else. It is not in
    /// the tree's source — see the test below for the measurement.
    ///
    /// It reads a line of the buffer the *shipped* renderer sees, which has no
    /// carriage returns (`the_shipped_order_…` and `endscrl.md` §6.2); the
    /// tree's file has one at the end of every line, so the caller strips them.
    fn shipped_advance(line: &[u8]) -> usize {
        match line {
            [b'[', digits @ ..] if !digits.is_empty() && digits.iter().all(u8::is_ascii_digit) => {
                core::str::from_utf8(digits).unwrap().parse().unwrap()
            }
            _ => 25,
        }
    }

    /// The ring row each text line *starts* at under that rule.
    fn shipped_ring_rows(text: &[u8]) -> Vec<usize> {
        let mut rows = vec![0usize];
        for line in text_lines_of(text) {
            rows.push(rows.last().unwrap() + shipped_advance(line));
        }
        rows
    }

    /// The row gaps between the *whole* bands of `lines`: the first and last
    /// are dropped, because the window's own ends cut the oldest and the newest
    /// line on screen.
    fn gaps_of(lines: &[(usize, usize)]) -> Vec<usize> {
        let whole = &lines[1..lines.len() - 1];
        whole.windows(2).map(|pair| pair[1].0 - pair[0].0).collect()
    }

    /// The columns of `frame` that hold ink anywhere in rows `top..=bottom`, as
    /// (first, width) runs — one per glyph, which is the geometry the font's
    /// metrics and the pen's arithmetic fix.
    fn column_runs(frame: &[u8], top: usize, bottom: usize) -> Vec<(usize, usize)> {
        let inked = |x: usize| {
            (top..=bottom).any(|y| {
                let at = (y * LINE_LEN + x) * 3;
                frame[at..at + 3] != [0, 0, 0]
            })
        };
        let mut runs: Vec<(usize, usize)> = Vec::new();
        let mut x = 0;
        while x < LINE_LEN {
            if !inked(x) {
                x += 1;
                continue;
            }
            let start = x;
            while x < LINE_LEN && inked(x) {
                x += 1;
            }
            runs.push((start, x - start));
        }
        runs
    }

    /// The first band's rows and its leftmost and rightmost inked columns.
    fn header(frame: &[u8]) -> (usize, usize, usize, usize) {
        let (top, bottom) = ink_bands(frame)[0];
        let mut left = VIEW_WIDTH as usize;
        let mut right = 0usize;
        for y in top..=bottom {
            let row = &frame[y * LINE_LEN * 3..][..LINE_LEN * 3];
            for x in 0..LINE_LEN {
                if row[x * 3..x * 3 + 3] != [0, 0, 0] {
                    left = left.min(x);
                    right = right.max(x);
                }
            }
        }
        (top, bottom, left, right)
    }

    #[test]
    fn the_references_text_runs_at_twenty_five_rows_a_line_where_the_source_says_thirty() {
        // **The reference is a different build, and this is the arithmetic.**
        // `MAIN.C:6` is `#define FONAY 30` and `:94` steps `line=(line+1)%FONAY`,
        // so a line of the text is 30 ring lines — 30 displayed rows — apart in
        // the tree's source. The reference's own frame says 25.
        //
        // Measured on `reference/parts/endscrl/frame_012000.rgb`, whose bands
        // are whole lines — `reference-indices.md` §21.2 identifies them as the
        // "Future Crew greets the following groups" block. Every gap between
        // the whole lines is a multiple of 25, and the smallest is 25; that
        // cannot be a fragment, because a line's ink is 18 rows tall
        // (`STRIP_INK_ROWS`) and the gaps are larger. The first and last bands
        // are dropped: the window's own ends cut the oldest and newest lines.
        let theirs = text_lines(&reference(12000));
        assert_eq!(theirs.len(), 12, "the bands of the reference's 12000");
        assert_eq!(theirs[0], (4, 17));
        let gaps = gaps_of(&theirs);
        assert_eq!(gaps, [25, 100, 50, 25, 25, 25, 25, 25, 25]);
        assert!(gaps.iter().all(|gap| gap % 25 == 0), "not whole 25-row lines: {gaps:?}");
        assert_eq!(gaps.iter().copied().min(), Some(25), "the pitch is the smallest gap");

        // The port, drawing the same text at the same scroll rate, at the tick
        // the reference's own 12000 lands on: every gap is a multiple of the 30
        // the source states, and none is 25.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 12000 - PORT_FIRST);
        let ours = text_lines(&rendered_frame(&rig));
        assert!(ours.len() >= 6, "the port's own frame shows a block: {ours:?}");
        let gaps = gaps_of(&ours);
        assert!(
            gaps.iter().all(|gap| gap % FONT_ROWS == 0),
            "not whole {FONT_ROWS}-row lines: {gaps:?}"
        );
        assert!(!gaps.contains(&25), "the port drew a 25-row line: {gaps:?}");
    }

    #[test]
    fn the_references_own_header_pins_the_chains_offset() {
        // Where the reference's scroller begins, and therefore how far ahead of
        // it the chain hands this part its tick 0.
        //
        // The measurement uses the one thing both builds agree on: the *first*
        // line of the text. For its first 24 lines of ring both draw the same
        // strip rows (`FONAY` only enters when the line counter comes round), so
        // the tick whose display puts "Second Reality" on the reference's own
        // rows is the tick that answers the reference's 9100 — and the offset
        // that tick sits at is the chain's, not the port's.
        let reference_9100 = reference(9100);
        let (top, bottom, left, right) = header(&reference_9100);
        assert_eq!((top, bottom, left, right), (309, 326, 219, 416));

        let (mut rig, mut part) = started();
        let mut matched: Vec<(u32, usize)> = Vec::new();
        for at in 0..=(9100 - REFERENCE_FIRST) {
            part.frame(&mut rig.ctx(at));
            // The chain's own bracket for the reference's start
            // (`reference-indices.md` §20/§21, `cred.md` §7) fixes the window:
            // the tick that answers 9100 is `9100 - start`.
            let low = 9100 - REFERENCE_LAST;
            let high = 9100 - REFERENCE_FIRST;
            if !(low..=high).contains(&at) {
                continue;
            }
            let (our_top, our_bottom, our_left, our_right) = header(&rendered_frame(&rig));
            // The port's line is one column left of the reference's: the tree's
            // text file holds a carriage return per line and the shipped
            // renderer's buffer does not (the centring is `(639-width)/2`, and
            // the CR's `fonaw` entry is 0, so it costs two columns). Measured as
            // a one-column shift rather than assumed.
            if (our_top, our_bottom) == (top, bottom)
                && (our_left, our_right) == (left - 1, right - 1)
            {
                matched.push((at, differing_pixels(&rig, &reference_9100, top, bottom, 1)));
            }
        }

        // A picture is two ticks wide (`:47`), so the header sits on its rows
        // for exactly a pair of ticks and nothing finer is observable.
        assert_eq!(matched.len(), 2, "the header matched on {matched:?}");
        assert_eq!(matched[1].0, matched[0].0 + 1, "the two ticks are not a pair");
        assert_eq!(matched[0].0, 183, "the tick whose header is on the reference's rows");

        // Where the reference's scroller begins, and what the chain's offset is:
        // the port's tick 0 is at index 8835, so the reference's start is 8917
        // (or 8918 — the pair) and the port is **82 frames early**, inside
        // CRED's own measured bracket (`cred.md` §7: 6 to 105 frames).
        let start = 9100 - matched[0].0;
        assert_eq!(start, 8917);
        assert!((REFERENCE_FIRST..=REFERENCE_LAST).contains(&start));
        assert_eq!(start - PORT_FIRST, 82);

        // The part's text can never run out inside the reference's own longest
        // run, which is why the scan's end-of-buffer case is unreachable:
        // 20,000 is this many lines of the text in, against the file's 542.
        assert_eq!((20_000 - start - 3) / 2 / FONT_ROWS as u32, 184);

        // And the remainder: 53 pixels over the band's 18 rows of 640 — 22 of
        // them ink, the rest values — which is the shipped build's raster, not
        // the geometry, metrics, palette or display, all of which this same
        // comparison exercises.
        assert_eq!(matched[0].1, 53, "the header band's differing pixels");
        assert_eq!(matched[1].1, 53);

        // The reference's own frame, measured once more, for the two claims the
        // note makes about it: the background is the cleared black (the part
        // writes none), and the *glyph geometry* is the tree's — the header's
        // ink columns are the same 13 runs on both sides once the carriage
        // return's one column is taken off.
        let black = reference_9100.chunks_exact(3).filter(|pixel| *pixel == [0, 0, 0]).count();
        assert_eq!(black, 252_312, "frame 9100's black pixels");
        let theirs = column_runs(&reference_9100, top, bottom);
        // The port's own frame at the matched tick again: the loop above left
        // the rig at the last tick it looked at.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, matched[0].0 + 1);
        let ours = column_runs(&rendered_frame(&rig), top, bottom);
        assert_eq!(theirs.len(), 13, "the header's glyph runs: {theirs:?}");

        // Of the header band's 53 differing pixels, 22 are ink on one side only
        // and the rest are two inks of different values — the shipped raster,
        // not the geometry (`the_shipped_order_…` is where the *widths* differ).
        let frame = rendered_frame(&rig);
        let (mut ink_only, mut values) = (0usize, 0usize);
        for y in top..=bottom {
            for x in 0..LINE_LEN - 1 {
                let at = (y * LINE_LEN + x) * 3;
                let mine = &frame[at..at + 3];
                let theirs = &reference_9100[at + 3..at + 6];
                if mine == theirs {
                    continue;
                }
                let mine_ink = mine != [0, 0, 0];
                let theirs_ink = theirs != [0, 0, 0];
                if mine_ink != theirs_ink {
                    ink_only += 1;
                } else {
                    values += 1;
                }
            }
        }
        assert_eq!((ink_only, values), (22, 31), "the header band's 53 pixels, split");

        let shifted: Vec<(usize, usize)> =
            theirs.iter().map(|(start, width)| (start - 1, *width)).collect();
        assert_eq!(ours, shifted, "the header's glyph runs are not the tree's");

        // The second line of the text, the same measurement: the reference's
        // starts at column 41 and the port's at 40 — the carriage return's
        // column, on the line that carries the most text of the two.
        let (second_top, second_bottom) = ink_bands(&reference_9100)[1];
        assert_eq!((second_top, second_bottom), (359, 376));
        let theirs = column_runs(&reference_9100, second_top, second_bottom);
        assert_eq!(theirs[0].0, 41, "the reference's second line starts here");
        let (our_top, our_bottom) = ink_bands(&rendered_frame(&rig))[1];
        assert_eq!((our_top, our_bottom), (369, 386), "the port's own pitch is 30 rows");
        let ours = column_runs(&rendered_frame(&rig), our_top, our_bottom);
        assert_eq!(ours[0].0, 40, "the port's second line starts here");
    }

    #[test]
    fn the_references_control_lines_advance_the_ring_and_draw_nothing() {
        // **The shipped build's dominant difference, and it is in the text.**
        // This tree's `MAIN.C` has no notion of a `[NN` line: `:66-74` reads a
        // line to its newline and `:77-88` draws whatever is in it, so a `[15`
        // line is drawn as the two digits (the `[` is not in `fonaorder`, so it
        // is a two-column gap). The reference does something else, and the
        // arithmetic names it: a `[NN` line **advances the ring NN rows and
        // draws nothing**, and every other line advances it 25 — not the
        // source's 30.
        //
        // The rule reproduces the reference's own bands to the row, on the text
        // file this tree ships, with no free parameter but the start measured in
        // `the_references_own_header_pins_the_chains_offset`: a line's ring row
        // is the sum of the advances before it, and it appears at screen row
        // `ring - n + 400`, `n` being the ring rows written by index K
        // (`n = (K - 8917 - 3) / 2 + 1`).
        let (_, part) = started();
        // The buffer the shipped renderer sees: the tree's file without the
        // carriage returns it holds (the port keeps them; §6.2).
        let text: Vec<u8> = part.text.iter().copied().filter(|byte| *byte != b'\r').collect();
        let lines = text_lines_of(&text);
        let rows = shipped_ring_rows(&text);

        // The control lines themselves: 163 of them, in four sizes, which is
        // exactly what the shipped text carries.
        let markers: Vec<usize> = lines
            .iter()
            .copied()
            .map(shipped_advance)
            .filter(|advance| *advance != 25)
            .collect();
        assert_eq!(markers.len(), 163, "the shipped text's control lines");
        let mut sizes = markers.clone();
        sizes.sort_unstable();
        sizes.dedup();
        assert_eq!(sizes, [8, 10, 12, 15], "the control lines' sizes");
        let count = |size: usize| markers.iter().filter(|marker| **marker == size).count();
        assert_eq!((count(8), count(10), count(12), count(15)), (6, 14, 135, 8));

        // The prediction, against the reference's own frames.
        for index in [10000u32, 14000] {
            let n: usize = (index as usize - 8917 - 3) / 2 + 1;
            let mut measured: Vec<usize> =
                text_lines(&reference(index)).iter().map(|(top, _)| *top).collect();
            measured.sort_unstable();
            let predicted: Vec<usize> = (0..lines.len())
                .filter(|line| !lines[*line].is_empty() && shipped_advance(lines[*line]) == 25)
                .filter_map(|line| {
                    let row = rows[line] as isize - n as isize + 400;
                    (0..400).contains(&row).then_some(row as usize)
                })
                .collect();
            assert!(!predicted.is_empty(), "frame {index}: nothing predicted");
            for row in &predicted {
                assert!(
                    measured.contains(row),
                    "frame {index}: the rule predicts a line at row {row}, and the reference \
                     has {measured:?} (n = {n})"
                );
            }
            // The window's own ends cut the oldest and the newest line on
            // screen, so at most two measured rows are ones the rule did not
            // predict.
            let extra: Vec<usize> =
                measured.iter().copied().filter(|row| !predicted.contains(row)).collect();
            assert!(extra.len() <= 2, "frame {index}: unexplained bands at {extra:?}");
        }

        // The readable numbers of the same measurement, where the record quotes
        // them. Frame 10000: the tree's own lines 16, 17, 19, 20 and 22, with a
        // `[15` between each pair, sit at ring rows 0, 25, 65, 90 and 130 —
        // 40 = 25 + 15 across each control line.
        assert_eq!(lines[16], b"Advanced Gravis");
        assert_eq!(lines[17], b"Canada");
        assert_eq!(lines[18], b"[15");
        assert_eq!(lines[19], b"Epic MegaGames");
        assert_eq!(lines[20], b"USA");
        assert_eq!(lines[21], b"[15");
        assert_eq!(lines[22], b"Waite Group Press");
        let n: usize = (10000 - 8917 - 3) / 2 + 1;
        let _ = n;
        for (line, ring_row) in [(16usize, 0usize), (17, 25), (19, 65), (20, 90), (22, 130)] {
            assert_eq!(rows[line] % 400, ring_row, "line {line}'s ring row");
        }
        assert_eq!(rows[19] - rows[17], 40);
        assert_eq!(rows[22] - rows[20], 40);

        // Frame 14000: six `[12` control lines between the personal greetings,
        // six 37-row steps (25 + 12).
        let n14000: usize = (14000 - 8917 - 3) / 2 + 1;
        let on_screen = |row: usize| row > n14000 - 400 && row <= n14000;
        let mut steps: Vec<usize> = Vec::new();
        for line in 0..lines.len() {
            // Both lines the control line sits between have to be on screen for
            // the step to be one the reference's frame shows.
            if shipped_advance(lines[line]) == 12
                && on_screen(rows[line - 1])
                && on_screen(rows[line + 1])
            {
                steps.push(rows[line + 1] - rows[line - 1]);
            }
        }
        assert_eq!(steps, [37, 37, 37, 37, 37, 37], "the six `[12` steps on screen");
        assert_eq!(lines[88], b"[12");
        assert_eq!(lines[89], b"Lord Cyrix / Access Denied");
        assert_eq!(lines[90], b"[12");

        // The `[10` and `[8` lines are **not** visible in any of this part's
        // four reference frames — their first is text line 430, and the deepest
        // line any keyframe reaches is 110 — so their 35- and 33-row steps
        // cannot be measured from what is committed, and this test does not
        // claim them.
        assert!(lines.iter().position(|line| shipped_advance(line) == 10).unwrap() > 400);
        assert!(lines.iter().position(|line| shipped_advance(line) == 8).unwrap() > 400);
        let deepest = (0..lines.len()).filter(|line| rows[*line] < (14000 - 8917 - 3) / 2 + 1).max();
        assert_eq!(deepest, Some(108), "the deepest line frame 14000 reaches");
    }

    #[test]
    fn the_shipped_order_names_six_characters_the_trees_does_not() {
        // The other half of the font difference, and the bigger half on the
        // frames that show it. This tree's `fonaorder` (`MAIN.C:13`) is 74
        // characters; the shipped build's is **80** — the packed
        // `MAIN/DATA/ENDSCRL.EXE` carries the tree's tail followed by the six
        // it adds:
        //
        //     $ xxd -s 9286 -l 40 upstream/SecondReality/MAIN/DATA/ENDSCRL.EXE
        //     00002446: 3637 3839 213f 2c2e 3a8f 8f28 292b 0000  6789!?,.:..()+..
        //     00002456: ff87 2d2a 3d27 5a84 9459 2f26 0065 2201  ..-*='Z..Y/&.e".
        //
        // (`Z`, `Y`, `/`, `&`, and the two high bytes `\x84` and `\x94`; the
        // blob is PKLITE-packed, so the run is interrupted by four bytes of the
        // stream between `()+` and `-*='`.) The shipped text uses five of them:
        // `/` 83 times, `&` 26, `Z` 12 times, `Y` once, and `\x84` (`ä`,
        // "Leppänen") twice. `fonaw` is 0 for every one of them here, so the
        // port draws a two-column gap and the reference draws a glyph.
        // The three counts the note and A11 quote, pinned here so they cannot
        // drift from the dump above: the tree's 74, the six the packed run
        // adds, and the 80 they make. (`0x94` is the one the shipped text never
        // asks for, which is why 'the text uses five' and 'the run is six' are
        // both true.)
        const ADDED: [u8; 6] = [b'Z', 0x84, 0x94, b'Y', b'/', b'&'];
        assert_eq!(FONT_ORDER.len(), 74, "the tree's own order");
        assert_eq!(FONT_ORDER.len() + ADDED.len(), 80, "the shipped build's order");

        let (mut _rig, mut part) = started();
        let text = part.text.clone();
        for absent in ADDED {
            let strip = part.strip.as_ref().unwrap();
            assert_eq!(strip.fonaw[usize::from(absent)], 0, "{absent} is in the tree's order");
        }
        assert_eq!(
            ADDED.iter().filter(|byte| text.contains(byte)).count(),
            5,
            "the shipped text asks for five of the six; 0x94 is unused"
        );
        let counts = |byte: u8| text.iter().filter(|ch| **ch == byte).count();
        assert_eq!(
            (counts(b'/'), counts(b'&'), counts(b'Z'), counts(b'Y'), counts(0x84)),
            (83, 26, 12, 1, 2),
            "what the shipped text asks for and the tree's order cannot draw"
        );

        // And what that costs, on frame 14000's own lines: the port's rendered
        // ink extent against the reference's band, line by line. The lines the
        // shipped order names characters in are the shipped glyph's own width
        // wider — 15 for `/`, 14 for `&` and for `ä` — and every line without
        // one is within the raster's single column.
        let frame = reference(14000);
        let bands: Vec<(usize, usize)> = text_lines(&frame);
        // The port's own rendering of a text line: `read_line` walks the
        // buffer to it, `render_line` draws each strip row of its ink, and the
        // extent is over all of them — the same measurement `column_runs` makes
        // of the reference's band.
        let mut rendered_width = |line: usize| -> usize {
            part.tptr = 0;
            for _ in 0..=line {
                part.read_line();
            }
            let (mut left, mut right) = (LINE_LEN, 0usize);
            for row in 0..STRIP_INK_ROWS {
                part.line = row;
                for (x, pixel) in part.render_line().iter().enumerate() {
                    if *pixel != 0 {
                        left = left.min(x);
                        right = right.max(x);
                    }
                }
            }
            right.saturating_sub(left)
        };
        for (band_row, line, difference) in [
            (28usize, 91usize, 15usize), // "Daryl / Sun Projects" — one `/`
            (65, 93, 14),                // "Tran & White Shadow" — one `&`
            (102, 95, 15),               // "Dr.Weird / DanTe" — one `/`
            (139, 97, 1),                // "Martti Palonen" — none: the raster's own column
            (176, 99, 1),                // "Jussi Markula" — none
            (213, 101, 1),               // "Mikko Vierula" — none
            (250, 103, 14),              // "Jussi Leppänen" — one `ä`
            (325, 106, 1),               // "Wildfire:" — none
            (375, 108, 14),              // "Emphii, Mace & Shaft" — one `&`
        ] {
            let (top, bottom) = *bands.iter().find(|(top, _)| *top == band_row).unwrap();
            let runs = column_runs(&frame, top, bottom);
            let theirs: usize = runs[runs.len() - 1].0 + runs[runs.len() - 1].1 - runs[0].0;
            let ours = rendered_width(line);
            assert_eq!(
                theirs - ours,
                difference,
                "line {line} at row {band_row}: the reference's band is {} columns wider",
                theirs - ours
            );
        }
    }
}
