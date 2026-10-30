//! ENDSCRL — the greetings scroller: a ring of text, and the beam walking it.
//!
//! **The port is the shipped build** (D49): `sr_assets::exe::unpack(assets/MAIN/DATA/ENDSCRL.EXE)`
//! gives a Borland C++ large-model image (DGROUP `0x1c890`), whose raster and `fonaorder` the part
//! reads and whose `main` (`0x1220-0x134a`), `do_scroll` (`0x134b-0x16ab`) and `init`
//! (`0x16ac-0x17b5`) it translates. The tree's `ENDSCRL/MAIN.C` is the same program with other
//! constants, cited where the two agree. The release differs in its raster (`font[30][1550]` at
//! `0x112e0`, scanned over 25 rows), an 80-character `fonaorder`, the blank (1530, 16), a 25-row
//! line, control lines (`[nn`: draw nothing for `nn` rows) and a `%` line that calls `exit(0)`
//! (`docs/fidelity/endscrl.md` §1). Both builds open the text without `O_BINARY`, so Borland's
//! `read` strips its carriage returns ([`read_text_mode`]).
//!
//! A tick is one `dis_waitb`; tick 0 is the code before the first:
//!
//! ```text
//! 0      0x1220-0x1225   dis_partstart and its wait: the screen is still CRED's
//! 1      0x122a-0x1326   mode 0eh, CRTC 9 = 0 (640x400), fifteen greys, init()
//! 2                      the loop's first dis_waitb, which returns 1
//! 3, 5…  0x132b-0x1348   do_scroll, one every two waits (reference-indices.md §21.1)
//! 22967                  the do_scroll that reads the `%` line: Finished
//! ```
//!
//! **The ring.** `do_scroll` writes each line at `80*yscrl` and again at `80*(yscrl+401)`, with
//! `yscrl = (yscrl+1) % 401` (`0x152d-0x164b`, `MAIN.C:89-95`), because the original's address
//! counter does not wrap. The port's beam does (`View::source_index` is modulo the surface, 400
//! lines of 640), so its ring is 400 lines written once, and row `j` at iteration `t` shows the
//! line written at `t - 399 + j` in both
//! (`tests::the_four_hundred_line_ring_shows_what_the_four_hundred_and_one_line_ring_shows`).
//! The origin reaches 256,000 pixels, which is why `start_addr` is a `u32` (D16).
//!
//! **The renderer** (`0x13e1-0x152a`, `MAIN.C:77-88`) walks the line a pixel at a time,
//! `mtau[x & 7]` picking the bit, and writes the strip value's four low bits into four planes at
//! one byte address: the port writes the value flat.
//!
//! The port's chain hands the part its tick 0 at 8835, the release's at 8917 (A11, A53).

use sr_assets::exe::{unpack, LoadImage};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

use crate::parts::read_asset;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "endscrl";

// ------------------------------------------------------------- the screen --

/// `0x122a-0x1235` (`MAIN.C:22-26`): mode 0eh is 640 pixels wide and 16 colours, and CRTC 9 = 0
/// is the 400-line display rather than the mode's own 350.
const VIEW_WIDTH: u16 = 640;
const VIEW_HEIGHT: u16 = 400;

/// `push 50h` before each plane's `memcpy` (`0x153b`): 80 bytes a line, one byte a plane, one
/// byte eight pixels.
const LINE_BYTES: usize = 80;
const LINE_LEN: usize = LINE_BYTES * 8;

/// The port's ring, in lines: 400 × 640 = 256,000 = the whole surface, and the wrap the
/// original's second copy stands in for (module doc).
const RING_LINES: usize = 400;

// ------------------------------------------------------------ the palette --

/// `0x1236-0x1323` (`MAIN.C:27-41`): `setrgbpalette(1,20,20,20)`, `(2,40,40,40)`, then
/// `(3..15, 60,60,60)` — fifteen calls, and entry 0 is not one of them.
const PALETTE_FIRST: u8 = 1;
const PALETTE_LAST: u8 = 15;
const PALETTE_GREYS: [u8; 15] = [20, 40, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60, 60];

// -------------------------------------------------------------- the image --

/// The shipped part, packed (PKLITE over LZEXE, `docs/fidelity/release-images.md`).
const EXE_PATH: &str = "assets/MAIN/DATA/ENDSCRL.EXE";
/// DGROUP's image offset: `mov dx,1c89h` at image 0, ×16.
const DGROUP: usize = 0x1c89 * 16;
/// The raster: `mov dx,112eh; mov es,dx` before every read of it (`0x1423`, `0x16fb`) — a far
/// array at segment `0x112e`, image `0x112e0`.
const FONT_AT: usize = 0x112e * 16;
/// The raster's row stride: `mov dx,060eh; imul dx` (`0x140a` in `do_scroll`, `0x16f3` in
/// `init`) — 1,550, where `MAIN.C:12` has 1,500. It is also the scan's bound
/// (`cmp word [bp-2],060eh` at `0x171c` and `0x175e`).
const FONT_STRIDE: usize = 0x60e;
/// The rows the scan reads (`cmp word [bp-4],19h` at `0x170d` and `0x174f`) and the rows a text
/// line draws (`mov bx,19h; idiv bx` at `0x165a`): 25, where `MAIN.C:6` has `FONAY` 30. No
/// other row of the raster is ever read.
const FONT_ROWS: usize = 0x19;
/// The array's own rows: `font[30][1550]` runs from `FONT_AT` to DGROUP, 46,512 bytes — 30 rows
/// of 1,550 rounded up to a paragraph.
const FONT_ARRAY_ROWS: usize = 30;
/// `fonaorder`, a far pointer at DGROUP `0x92` (`les bx,[0092h]` at `0x1765`): its offset word
/// and its segment word, which the image holds as `00b2 1c89`.
const FONAORDER_PTR: usize = DGROUP + 0x92;
/// The `char far text[64000]` the text is read into: `push 018eh; push 0` at `0x16c6-0x16c9`,
/// segment `0x18e`. Its initial bytes are the array's initialiser, which the image holds.
const TEXT_AT: usize = 0x18e * 16;
/// `MAIN.C:52`'s `text[64000]`: the array runs from `TEXT_AT` to the raster's segment.
const TEXT_BUFFER: usize = FONT_AT - TEXT_AT;
/// `push 0ea60h` at `0x16c3`: `read(fd, text, 60000)`.
const TEXT_READ_MAX: usize = 0xea60;
/// `mov word [43a8h],05fah` at `0x17a8`: `fonap[' '] = 1530` (`fonap` is DGROUP `0x4368`,
/// `0x43a8` its entry 32). The raster's columns `1530..1546` hold no ink, so the blank draws
/// nothing and advances the pen.
const BLANK: u8 = b' ';
const BLANK_COLUMN: usize = 0x5fa;
/// `mov word [45a8h],10h` at `0x17ae`: `fonaw[' '] = 16` (`fonaw` is DGROUP `0x4568`).
const BLANK_WIDTH: usize = 0x10;

// --------------------------------------------------------------- the text --

/// The shipped greetings text: `open` names `"endscrol.txt"` (DGROUP `0x103`, `push 0103h` at
/// `0x16b5`), which the shell's `int 21h` hook served from the packed file — 6,141 bytes,
/// `MAIN/PACKFINA.INC:51` — which is `MAIN/DATA/ENDSCROL.TXT`. The 1,007-byte
/// `ENDSCRL/ENDSCROL.TXT` beside the tree's source is the developer's own test text.
const TEXT_PATH: &str = "assets/MAIN/DATA/ENDSCROL.TXT";
/// Borland's text-mode `read` (`0xc62`): a carriage return is dropped (`cmp al,0dh` at
/// `0xcc5`) and a `^Z` is the end of the file (`cmp al,1ah` at `0xcc1`).
const CR: u8 = b'\r';
const CTRL_Z: u8 = 0x1a;
/// `cmp byte es:[bx],0ah` at `0x139f` (`MAIN.C:68`): the text is line-delimited, and the
/// newline itself is copied into `textline` (`0x13a5-0x13ab`) and never drawn.
const LINE_END: u8 = b'\n';
/// `mov ax,027fh; sub ax,[009ah]; … idiv` at `0x13b3-0x13be` (`MAIN.C:73`):
/// `tstart = (639 - tstart) / 2`.
const TEXT_CENTRE: i32 = 0x27f;
/// `cmp byte [0484h],5bh` at `0x13c3` and `0x164f`: a line whose first byte is `[` is a
/// control line.
const CONTROL: u8 = b'[';
/// `cmp byte [0484h],25h` at `0x169a`: a line whose first byte is `%` ends the part.
const END: u8 = b'%';
/// `mov bx,19h` at `0x165a`: every other line lasts 25 rows.
const LINE_ROWS: i32 = 0x19;
/// `MAIN.C:57`'s `char textline[100]` (DGROUP `0x484`).
const TEXTLINE_LEN: usize = 100;
/// `int mtau[8]={128,64,32,16,8,4,2,1}` (DGROUP `0x9e`, `MAIN.C:55`): `x & 7` picks bit
/// `7 - (x & 7)`, so the leftmost pixel of a byte is its high bit.
const MTAU: [u8; 8] = [128, 64, 32, 16, 8, 4, 2, 1];
/// `scanbuf[4][80]`: four planes, one line each.
const PLANES: usize = 4;

// --------------------------------------------------------------- the loop --

/// The tick the mode set lands on, and the tick of the first `do_scroll` — see the module
/// docs' tick map. The two waits the loop spends per iteration put every later line two ticks
/// after the last.
const MODE_TICK: u32 = 1;
const FIRST_SCROLL_TICK: u32 = 3;

// -------------------------------------------------------------- the strip --

/// What `init` reads out of the image: the raster, and the two tables its scan fills.
struct Strip {
    /// The raster's first [`FONT_ROWS`] rows, [`FONT_STRIDE`] bytes each.
    raster: Vec<u8>,
    /// `fonap[256]` (DGROUP `0x4368`): each character's first column, 0 for one the order
    /// does not name.
    fonap: [usize; 256],
    /// `fonaw[256]` (DGROUP `0x4568`): each character's width, 0 for one the order does not
    /// name.
    fonaw: [usize; 256],
}

impl Strip {
    /// `init`'s scan, `0x16e1-0x17ae` (`MAIN.C:103-124`, at the image's bounds).
    ///
    /// `x` walks the raster's 1,550 columns while the order has characters left: blank
    /// columns are skipped, the next run of inked ones is the next character's, and its first
    /// column and width go into `fonap` and `fonaw`. A column is inked when any of its first 25
    /// rows is non-zero. The characters are bytes read unsigned (`mov ah,0` at `0x176c`), so
    /// `\x84` and `\x94` index entries 132 and 148.
    fn scan(raster: Vec<u8>, order: &[u8]) -> Strip {
        let inked = |x: usize| (0..FONT_ROWS).any(|y| raster[y * FONT_STRIDE + x] != 0);
        let mut fonap = [0usize; 256];
        let mut fonaw = [0usize; 256];
        let mut x = 0;
        let mut next = order.iter();
        // `cmp word [bp-2],060eh; jge` then `cmp byte es:[bx],0; je` at `0x1794-0x17a3`.
        while x < FONT_STRIDE {
            let Some(&character) = next.as_slice().first() else {
                break;
            };
            next.next();
            while x < FONT_STRIDE && !inked(x) {
                x += 1;
            }
            let start = x;
            while x < FONT_STRIDE && inked(x) {
                x += 1;
            }
            fonap[usize::from(character)] = start;
            fonaw[usize::from(character)] = x - start;
        }
        fonap[usize::from(BLANK)] = BLANK_COLUMN;
        fonaw[usize::from(BLANK)] = BLANK_WIDTH;
        Strip { raster, fonap, fonaw }
    }
}

/// The two things `init` reads out of the image: the raster's first [`FONT_ROWS`] rows, and
/// `fonaorder` (the bytes up to its NUL) — and the text array's own initial bytes.
struct ImageData {
    raster: Vec<u8>,
    order: Vec<u8>,
    text: Vec<u8>,
}

/// Read [`ImageData`] out of an unpacked image, refusing one whose layout is not this build's.
fn image_data(image: &LoadImage) -> Result<ImageData, String> {
    let bytes = &image.image;
    let word = |at: usize| -> Result<usize, String> {
        match bytes.get(at..at + 2) {
            Some(pair) => Ok(usize::from(u16::from_le_bytes([pair[0], pair[1]]))),
            None => Err(format!("the image ends before {at:#x}")),
        }
    };
    // The far pointer `fonaorder`, as the image holds it before relocation: the segment word is
    // relative to the load segment, which is the image's own origin.
    let order_at = word(FONAORDER_PTR + 2)? * 16 + word(FONAORDER_PTR)?;
    let order_len = bytes
        .get(order_at..)
        .and_then(|tail| tail.iter().position(|&byte| byte == 0))
        .ok_or_else(|| format!("fonaorder at {order_at:#x} has no terminator"))?;
    let raster_end = FONT_AT + FONT_ROWS * FONT_STRIDE;
    if bytes.len() < raster_end || FONT_AT + FONT_ARRAY_ROWS * FONT_STRIDE > DGROUP {
        return Err(format!("the image is {} bytes; its raster ends at {raster_end:#x}", bytes.len()));
    }
    // A relocation inside what is read would be a segment word, not data.
    let read = [(TEXT_AT, raster_end), (order_at, order_at + order_len)];
    if let Some(at) = image
        .relocations
        .iter()
        .map(|&at| at as usize)
        .find(|at| read.iter().any(|(from, to)| (*from..*to).contains(at)))
    {
        return Err(format!("a relocation at {at:#x} lies inside the data read"));
    }
    Ok(ImageData {
        raster: bytes[FONT_AT..raster_end].to_vec(),
        order: bytes[order_at..order_at + order_len].to_vec(),
        text: bytes[TEXT_AT..TEXT_AT + TEXT_BUFFER].to_vec(),
    })
}

/// Borland's `read` on a text-mode handle (`0xc62`): the bytes DOS returns are read into
/// `buffer`, then compacted **in place** (`lodsb`/`stosb` at `0xcbf-0xcc9`) — a carriage return
/// skipped, a `^Z` ending the file — and the count kept is returned. The bytes past it are the
/// raw read's own, left where they were.
///
/// One case is not modelled: a carriage return that is the read's *last* byte makes the
/// runtime read one more byte (`0xcd0-0xcec`). The shipped text ends in a newline
/// (`the_text_is_read_in_text_mode_without_its_carriage_returns` pins it).
fn read_text_mode(file: &[u8], buffer: &mut [u8]) -> usize {
    let read = file.len().min(TEXT_READ_MAX).min(buffer.len());
    buffer[..read].copy_from_slice(&file[..read]);
    let mut kept = 0;
    for at in 0..read {
        let byte = buffer[at];
        if byte == CTRL_Z {
            break;
        }
        if byte != CR {
            buffer[kept] = byte;
            kept += 1;
        }
    }
    kept
}

/// How many rows the line in `textline` lasts: `0x164f-0x1681`. A control line's count is
/// `(textline[1]-'0')*10 + textline[2]-'0'`, in 16-bit arithmetic on the bytes read unsigned —
/// so `[12` is 12, and `[8`, whose third byte is the newline, is `8*10 + 10-48` = **42**.
fn line_rows(textline: &[u8]) -> i32 {
    if textline[0] != CONTROL {
        return LINE_ROWS;
    }
    (i32::from(textline[1]) - i32::from(b'0')) * 10 + i32::from(textline[2]) - i32::from(b'0')
}

// ---------------------------------------------------------------- the part --

/// ENDSCRL: the ring, the pen and the beam.
pub struct Endscrl {
    /// The strip, once `init` has read it.
    strip: Option<Strip>,
    /// `text[64000]`: the array's own initial bytes, overwritten at the front by `read`.
    text: Vec<u8>,
    /// `tptr`'s offset (DGROUP `0x96`).
    tptr: usize,
    /// `textline` (DGROUP `0x484`), written from index 0 for each line and never cleared, so
    /// bytes past a short line are the last longer line's — the source's own static array.
    /// It grows rather than overflowing; the shipped text's longest line is 40 characters.
    textline: Vec<u8>,
    /// `chars` (DGROUP `0x9c`).
    chars: usize,
    /// `tstart` (DGROUP `0x9a`): the pen's first column on the line.
    tstart: i32,
    /// `yscrl` (DGROUP `0xae`), in the port's 400-line ring.
    yscrl: usize,
    /// `line` (DGROUP `0xb0`): the strip row it is drawn from, or a control line's count.
    line: i32,
    /// The part's own tick, which is the shell's count for this part.
    tick: u32,
    /// Whether the `%` line has been reached: the part has called `exit`.
    ended: bool,
}

impl Endscrl {
    /// A part with nothing loaded: `init` reads the image and the text.
    pub fn new() -> Endscrl {
        Endscrl {
            strip: None,
            text: Vec::new(),
            tptr: 0,
            textline: vec![0; TEXTLINE_LEN],
            chars: 0,
            tstart: 0,
            yscrl: 0,
            line: 0,
            tick: 0,
            ended: false,
        }
    }

    /// `0x122a-0x1326` (`MAIN.C:22-43`) — the mode, the palette, `init`, all of it one tick.
    fn enter_mode(&self, ctx: &mut PartCtx) {
        // Mode 0eh, then CRTC 9 = 0. The mode set is also what clears the video memory
        // (`int10_modes.cpp`'s `finish_set_mode`, `:813` and `:859`), and the memory it clears
        // is CRED's: this is why the ring's unwritten lines are black.
        ctx.surface.clear(0);
        // Fifteen `setrgbpalette` calls. Entry 0 and 16-255 are left where the previous part
        // left them — the reference's own frames show entry 0 black.
        for entry in PALETTE_FIRST..=PALETTE_LAST {
            let grey = PALETTE_GREYS[usize::from(entry - PALETTE_FIRST)];
            ctx.set_palette_entry(entry, [grey, grey, grey]);
        }
        Endscrl::set_display(ctx, 0);
    }

    /// The display the mode set leaves and `setstart` moves: 640 pixels a row, 400 rows, no
    /// doubling, no split, and the beam's origin.
    fn set_display(ctx: &mut PartCtx, origin: u32) {
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        // The display's line stride *is* the ring's line — 640 pixels, which is also the
        // surface's own row stride. Written out because the part addresses the ring itself.
        ctx.view.y_line_offset = Some(LINE_LEN as u16);
        ctx.view.double_scan = false; // CRTC 9 = 0
        ctx.view.pan = 0;
        ctx.view.split = None;
        ctx.view.set_origin(origin);
    }

    /// `0x1351-0x13ca` (`MAIN.C:66-74`) — the next line of the text, and where it starts.
    ///
    /// Characters up to the newline, each adding `fonaw[c] + 2` to the line's width
    /// (`0x1386-0x138c`); the newline copied into `textline` and stepped over (`0x13a5-0x13af`);
    /// the line centred by `(639 - width) / 2`, a truncating `idiv`; and then, the shipped
    /// build's own step, `chars = 0` for a control line (`0x13c3-0x13ca`), which is what makes
    /// it draw nothing.
    fn read_line(&mut self) {
        let mut width = 0i32;
        let mut a = 0usize;
        // The source's buffer has no terminator if the text runs out; the port stops at the
        // buffer's end, which the shipped text never reaches: its `%` line ends the part first.
        while self.tptr < TEXT_BUFFER && self.text[self.tptr] != LINE_END {
            let character = self.text[self.tptr];
            self.put_textline(a, character);
            width += self.strip.as_ref().expect("init read the strip").fonaw
                [usize::from(character)] as i32
                + 2;
            self.tptr += 1;
            a += 1;
        }
        let newline = self.text.get(self.tptr).copied().unwrap_or(0);
        self.put_textline(a, newline);
        self.tptr += 1;
        self.chars = a;
        self.tstart = (TEXT_CENTRE - width) / 2;
        if self.textline[0] == CONTROL {
            self.chars = 0;
        }
    }

    /// `textline[a] = c`, growing the port's array where the source's would overflow.
    fn put_textline(&mut self, at: usize, character: u8) {
        if at >= self.textline.len() {
            self.textline.resize(at + 1, 0);
        }
        self.textline[at] = character;
    }

    /// `0x13d0-0x152a` (`MAIN.C:75-88`) — the line, as the 640 pixels its four planes spell.
    ///
    /// `scanbuf` is four rows of 80 bytes, cleared (`memset` at `0x13d9`); the pen walks the
    /// line one pixel at a time from `tstart`, two columns of gap after each character
    /// (`add word [bp-6],2` at `0x151d`), reading `font[line*1550 + fonap[c] + b]`
    /// (`0x1407-0x142a`), and the value's four low bits go one per plane. The port folds the
    /// four planes into the flat value its surface holds.
    fn render_line(&self) -> [u8; LINE_LEN] {
        let strip = self.strip.as_ref().expect("init read the strip");
        let mut scanbuf = [0u8; PLANES * LINE_BYTES];
        let mut x = self.tstart;
        for &character in &self.textline[..self.chars] {
            let column = strip.fonap[usize::from(character)];
            for b in 0..strip.fonaw[usize::from(character)] {
                let value = strip.raster[self.line as usize * FONT_STRIDE + column + b];
                // `scanbuf[plane][x/8] ^= mtau[x & 7]`. The shipped text keeps `x` inside the
                // row (`the_shipped_text_fits_the_row_and_the_textline_buffer`); the original
                // writes outside `scanbuf` if it does not, so the port drops those pixels.
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
            x += 2;
        }

        // The four plane `memcpy`s, resolved: the four planes are the pixel's four bits.
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

    /// `do_scroll`, `0x134b-0x16ab` — one line of the text, the display start moved on, and
    /// the `%` exit.
    fn do_scroll(&mut self, ctx: &mut PartCtx) {
        if self.line == 0 {
            self.read_line();
        }
        let row = self.render_line();
        // `0x152d-0x163e`: the line, per plane, at `80*yscrl` — and again at
        // `80*(yscrl+401)` (`add ax,191h` at `0x1561`), which the port's wrapping beam
        // stands in for (module doc).
        let at = self.yscrl * LINE_LEN;
        ctx.surface.pixels_mut()[at..at + LINE_LEN].copy_from_slice(&row);

        // `0x1641-0x164b`: `yscrl = (yscrl+1) % 401`, in the port's 400-line ring.
        let origin = (self.yscrl + 1) * LINE_LEN;
        self.yscrl = (self.yscrl + 1) % RING_LINES;
        // `0x164f-0x1683`: `line = (line+1) % 25`, or a control line's own count.
        self.line = (self.line + 1) % line_rows(&self.textline);
        // `0x1687-0x1693`: `setstart(yscrl*80 + 80)` with the post-increment `yscrl` —
        // `(yscrl + 1) * 640` in the port's ring, for the `yscrl` before the step.
        Endscrl::set_display(ctx, origin as u32);
        // `0x169a-0x16a3`: `if (textline[0] == '%') exit(0);` — `0x272` is the C runtime's
        // `exit`, the `atexit` loop over DGROUP `0x110`/`0x4768` and then `_exit`.
        if self.textline[0] == END {
            self.ended = true;
        }
    }
}

impl Part for Endscrl {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `init`, `0x16ac-0x17b5`: the text into the buffer, and the strip's metrics — both
    /// read out of the shipped image.
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        let error = |message: String| PartError { part: NAME, message };
        let file = read_asset(NAME, EXE_PATH)?;
        let image = unpack(&file).map_err(|e| error(format!("{EXE_PATH}: {e}")))?;
        let data = image_data(&image).map_err(|e| error(format!("{EXE_PATH}: {e}")))?;
        self.strip = Some(Strip::scan(data.raster, &data.order));

        // `open("endscrol.txt", 1)`, `read(fd, text, 60000)`, `close` (`0x16b2-0x16da`): the
        // text-mode read drops the carriage returns.
        let text = read_asset(NAME, TEXT_PATH)?;
        self.text = data.text;
        read_text_mode(&text, &mut self.text);
        // Every control line the part reaches divides by its own count (`idiv bx` at
        // `0x1681`); a count of 0 would be the original's divide error, so it is refused here.
        for line in self.text.split(|byte| *byte == LINE_END) {
            if line.first() == Some(&END) {
                break;
            }
            if line.first() == Some(&CONTROL) {
                let mut textline = line.to_vec();
                textline.extend_from_slice(&[LINE_END, 0]);
                if line_rows(&textline) <= 0 {
                    return Err(error(format!(
                        "{TEXT_PATH}: the control line {:?} lasts no rows",
                        String::from_utf8_lossy(line)
                    )));
                }
            }
        }
        Ok(())
    }

    /// One tick of `main`'s loop. The module doc's tick map.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.ended {
            return PartStatus::Finished;
        }
        match self.tick {
            // `0x1220-0x1225`: the part's own blank. The screen is still CRED's.
            0 => {}
            MODE_TICK => self.enter_mode(ctx),
            tick if tick >= FIRST_SCROLL_TICK && (tick - FIRST_SCROLL_TICK) % 2 == 0 => {
                self.do_scroll(ctx)
            }
            // The loop's second wait: a tick with nothing drawn on it, which is what makes the
            // scroll step two frames wide.
            _ => {}
        }
        self.tick += 1;
        // `while(!dis_exit())` is the shell's to see (ESC). The `%` line is the part's own end.
        if self.ended {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{differing_pixels, reference, Rig};
    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Endscrl) {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT);
        let mut part = Endscrl::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// The unpacked image, as the part reads it.
    fn image() -> LoadImage {
        unpack(&read_asset(NAME, EXE_PATH).unwrap()).unwrap()
    }

    /// Runs the part up to (and not including) `tick`, numbering each call by the part's own
    /// tick count — which is what the shell does.
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

    /// The reference's own CRED→ENDSCRL bracket, measured in the captures: the sweep's log
    /// shows 8840 still CRED's screen and refuses 8940 as `640x400`, and the geometry change
    /// is this part's tick 2, so the start lies in **`[8841, 8938]`** — `reference-indices.md`
    /// §20/§21, `docs/fidelity/cred.md` §7.
    const REFERENCE_FIRST: u32 = 8841;
    const REFERENCE_LAST: u32 = 8938;

    /// Where the port's chain hands this part its tick 0 — measured through the binary in
    /// `tests/endscrl.rs` from the dump's own `part` fields.
    const PORT_FIRST: u32 = 8835;

    /// Where the release's chain hands it its tick 0, measured below from the reference's
    /// own frame 9100 (`the_references_own_header_pins_the_chains_offset`).
    const RELEASE_FIRST: u32 = 8917;

    /// The keyframes `reference/parts/endscrl` holds.
    const KEYFRAMES: [u32; 4] = [9100, 10000, 12000, 14000];

    /// The port's frame at the release's index `index`: the picture after the part's tick
    /// `index - RELEASE_FIRST`.
    fn at_release_index(index: u32) -> Vec<u8> {
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, index - RELEASE_FIRST + 1);
        rig.rendered_rgb()
    }

    /// The rows of `frame` that hold ink, as (first, last) pairs — one per run of consecutive
    /// rows.
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

    /// The text as the part holds it, split into its lines — `read_line`'s own unit.
    fn text_lines_of(text: &[u8]) -> Vec<&[u8]> {
        text.split(|byte| *byte == LINE_END).collect()
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

    #[test]
    fn the_image_is_the_build_these_offsets_name() {
        // The offsets the part reads data at, read back as the instructions that name them.
        let image = image();
        let at = |offset: usize, bytes: &[u8]| {
            assert_eq!(&image.image[offset..offset + bytes.len()], bytes, "image {offset:#x}");
        };
        at(0x0000, &[0xba, 0x89, 0x1c]); // mov dx,1c89h: DGROUP
        // main: mode 0eh, CRTC 9 = 0.
        at(0x122a, &[0xb8, 0x0e, 0x00, 0xcd, 0x10]);
        at(0x122f, &[0xb8, 0x09, 0x00, 0xba, 0xd4, 0x03, 0xef]);
        // The fifteen `setrgbpalette(i, g, g, g)` calls, sixteen bytes each.
        for (k, grey) in PALETTE_GREYS.iter().enumerate() {
            let i = PALETTE_FIRST + k as u8;
            at(
                0x1236 + 16 * k,
                &[0x6a, *grey, 0x6a, *grey, 0x6a, *grey, 0x6a, i, 0x9a, 0x29, 0x00, 0x7b, 0x01],
            );
        }
        // do_scroll: the centre, the control line's `chars = 0`, the raster's stride and
        // segment, `add ax,191h`, the 25-row line, the control count, the `%` exit.
        at(0x13b3, &[0xb8, 0x7f, 0x02]);
        at(0x13c3, &[0x80, 0x3e, 0x84, 0x04, CONTROL, 0x75, 0x06, 0xc7, 0x06, 0x9c, 0x00, 0, 0]);
        at(0x140a, &[0xba, 0x0e, 0x06, 0xf7, 0xea]);
        at(0x1423, &[0xba, 0x2e, 0x11]);
        at(0x1561, &[0x05, 0x91, 0x01]);
        at(0x164f, &[0x80, 0x3e, 0x84, 0x04, CONTROL, 0x74, 0x0c]);
        at(0x165a, &[0xbb, 0x19, 0x00, 0x99, 0xf7, 0xfb]);
        at(0x1662, &[0xa0, 0x85, 0x04, 0xb4, 0x00, 0x05, 0xd0, 0xff, 0xba, 0x0a, 0x00, 0xf7, 0xea]);
        at(0x169a, &[0x80, 0x3e, 0x84, 0x04, END, 0x75, 0x09, 0x6a, 0x00]);
        // init: open(name, 1), read(fd, 018e:0000, 60000), the raster's stride and segment,
        // the scan's bounds, the blank.
        at(0x16b2, &[0x6a, 0x01, 0x1e, 0x68, 0x03, 0x01]);
        at(0x16c3, &[0x68, 0x60, 0xea, 0x68, 0x8e, 0x01, 0x68, 0x00, 0x00]);
        at(0x16f3, &[0xba, 0x0e, 0x06]);
        at(0x16fb, &[0xba, 0x2e, 0x11]);
        at(0x170d, &[0x83, 0x7e, 0xfc, 0x19]);
        at(0x174f, &[0x83, 0x7e, 0xfc, 0x19]);
        at(0x171c, &[0x81, 0x7e, 0xfe, 0x0e, 0x06]);
        at(0x175e, &[0x81, 0x7e, 0xfe, 0x0e, 0x06]);
        at(0x1765, &[0xc4, 0x1e, 0x92, 0x00]);
        at(0x17a8, &[0xc7, 0x06, 0xa8, 0x43, 0xfa, 0x05]);
        at(0x17ae, &[0xc7, 0x06, 0xa8, 0x45, 0x10, 0x00]);
        // The runtime's `read`: the CR skip and the ^Z stop, and `_fmode`'s O_TEXT.
        at(0x0cbf, &[0x26, 0xac, 0x3c, CTRL_Z, 0x74, 0x30, 0x3c, CR, 0x74, 0x05, 0xaa]);
        at(DGROUP + 0x2d8, &[0x00, 0x40]);
        // The file name, and `fonaorder`'s far pointer.
        at(DGROUP + 0x103, b"endscrol.txt\0");
        at(FONAORDER_PTR, &[0xb2, 0x00, 0x89, 0x1c]);
    }

    #[test]
    fn the_strip_and_its_order_are_read_from_the_image() {
        let image = image();
        let data = image_data(&image).unwrap();
        // `fonaorder`: the tree's 74 characters (`MAIN.C:13`) and six more, at DGROUP 0xb2.
        assert_eq!(
            data.order,
            b"ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='Z\x84\x94Y/&"
        );
        assert_eq!(data.raster.len(), 25 * 1550);

        let (_, part) = started();
        let strip = part.strip.as_ref().unwrap();
        // One run per character: 80 runs, every character drawn — the two `\x8f`s share an
        // entry, the second overwriting the first, as the scan does.
        for &character in &data.order {
            assert!(strip.fonaw[usize::from(character)] > 0, "{character:#x} has no run");
        }
        let at = |c: u8| (strip.fonap[usize::from(c)], strip.fonaw[usize::from(c)]);
        assert_eq!(at(b'A'), (1, 20), "the first run");
        assert_eq!(at(0x8f), (942, 3), "the second \\x8f's run");
        assert_eq!(
            [at(b'Z'), at(0x84), at(0x94), at(b'Y'), at(b'/'), at(b'&')],
            [(1012, 16), (1029, 13), (1043, 11), (1055, 17), (1073, 14), (1088, 13)],
            "the six characters the tree's order does not have"
        );
        // The text uses five of the six.
        let count = |byte: u8| part.text[..5_599].iter().filter(|c| **c == byte).count();
        assert_eq!(
            [count(b'/'), count(b'&'), count(b'Z'), count(b'Y'), count(0x84), count(0x94)],
            [83, 26, 12, 1, 2, 0]
        );
        let widths: Vec<usize> = data.order.iter().map(|c| strip.fonaw[usize::from(*c)]).collect();
        assert_eq!((widths.iter().min(), widths.iter().max()), (Some(&3), Some(&25)));
        // The blank: sixteen columns from 1530 that hold no ink in any row.
        assert_eq!(at(BLANK), (1530, 16));
        assert!((0..FONT_ROWS).all(|y| strip.raster[y * FONT_STRIDE + 1530..][..16]
            .iter()
            .all(|v| *v == 0)));
        // A character the order does not name has no width: `[`, `%`, the newline.
        for absent in [CONTROL, END, LINE_END, CR, 0x00] {
            assert_eq!(strip.fonaw[usize::from(absent)], 0, "{absent:#x}");
        }
        // The raster's values are 2-bit, which the four plane tests read; its ink is in rows
        // 0-20; and rows 25-29 of the `[30][1550]` array, which nothing reads, hold none.
        assert!(strip.raster.iter().all(|v| *v < 4));
        let inked_rows: Vec<usize> = (0..FONT_ROWS)
            .filter(|y| strip.raster[y * FONT_STRIDE..][..FONT_STRIDE].iter().any(|v| *v != 0))
            .collect();
        assert_eq!(inked_rows, (0..=20).collect::<Vec<_>>());
        assert_eq!((DGROUP - FONT_AT) / 16, (FONT_ARRAY_ROWS * FONT_STRIDE).div_ceil(16));
        assert!(image.image[FONT_AT + FONT_ROWS * FONT_STRIDE..FONT_AT + FONT_ARRAY_ROWS * FONT_STRIDE]
            .iter()
            .all(|v| *v == 0));
    }

    #[test]
    fn the_text_is_read_in_text_mode_without_its_carriage_returns() {
        // `open("endscrol.txt", 1)` takes `_fmode`'s O_TEXT, and Borland's `read` compacts the
        // bytes in place: 542 carriage returns dropped from the 6,141 bytes, 5,599 kept. The
        // bytes after the kept ones are the raw read's own, and the rest of the array is its
        // initialiser, which the image holds (the tree's `"Testing Testing\0…"`, `MAIN.C:52`).
        let shipped = read_asset(NAME, TEXT_PATH).unwrap();
        assert_eq!(shipped.len(), 6_141);
        assert_eq!(shipped.iter().filter(|b| **b == CR).count(), 542);
        assert!(!shipped.contains(&CTRL_Z));
        assert!(shipped.ends_with(b"\r\n"), "the one read case not modelled: a CR last");
        let stripped: Vec<u8> = shipped.iter().copied().filter(|b| *b != CR).collect();
        assert_eq!(stripped.len(), 5_599);

        let (_, part) = started();
        assert_eq!(part.text.len(), 64_000);
        assert_eq!(&part.text[..5_599], &stripped[..]);
        assert_eq!(&part.text[5_599..6_141], &shipped[5_599..], "the raw read's tail");
        let initial = &image().image[TEXT_AT..TEXT_AT + TEXT_BUFFER];
        assert!(initial.starts_with(b"Testing Testing\0PIXEL SUCKS\0"));
        assert_eq!(&part.text[6_141..], &initial[6_141..]);
        assert!(part.text.starts_with(b"Second Reality\n"));

        // The read itself, on a buffer it fills less than: a CR dropped, a ^Z ending it.
        let mut buffer = [9u8; 8];
        assert_eq!(read_text_mode(b"a\r\nb\x1ac", &mut buffer), 3);
        assert_eq!(buffer, [b'a', b'\n', b'b', b'b', CTRL_Z, b'c', 9, 9]);
    }

    #[test]
    fn the_shipped_text_fits_the_row_and_the_textline_buffer() {
        // `textline` is 100 bytes and `scanbuf` 80 bytes a plane, both written without a
        // bound. The shipped text stays inside them.
        let (_, part) = started();
        let strip = part.strip.as_ref().unwrap();
        let lines = text_lines_of(&part.text[..5_599]);
        assert_eq!(lines.len(), 543);
        assert!(lines[542].is_empty(), "the text ends in a newline");
        let width =
            |line: &[u8]| -> i32 { line.iter().map(|c| strip.fonaw[usize::from(*c)] as i32 + 2).sum() };
        for (at, line) in lines[..542].iter().enumerate() {
            let tstart = (TEXT_CENTRE - width(line)) / 2;
            assert!(tstart >= 0, "line {at} does not fit");
            assert!(tstart + width(line) < LINE_LEN as i32, "line {at} runs past the row");
            assert!(line.len() < TEXTLINE_LEN, "line {at}: {} characters", line.len());
        }
        assert_eq!(lines.iter().map(|line| line.len()).max(), Some(40));
        let widest = lines.iter().map(|line| width(line)).max().unwrap();
        assert_eq!(widest, 592, "the widest line");
        assert_eq!((TEXT_CENTRE - widest) / 2, 23, "its first column");
        assert_eq!((TEXT_CENTRE - widest) / 2 + widest - 3, 612, "its last column");
    }

    #[test]
    fn the_references_own_header_pins_the_chains_offset() {
        // Where the release's scroller begins, and therefore how far ahead of it the port's
        // chain hands this part its tick 0. The tick whose display puts "Second Reality" on
        // the reference's own rows of frame 9100, inside the chain's bracket, is the tick that
        // answers 9100.
        let reference_9100 = reference("endscrl", 9100, "rgb");
        let (top, bottom, left, right) = header(&reference_9100);
        assert_eq!((top, bottom, left, right), (309, 326, 219, 416));

        let (mut rig, mut part) = started();
        let mut matched: Vec<(u32, usize)> = Vec::new();
        for at in 0..=(9100 - REFERENCE_FIRST) {
            part.frame(&mut rig.ctx(at));
            if !(9100 - REFERENCE_LAST..=9100 - REFERENCE_FIRST).contains(&at) {
                continue;
            }
            let frame = rig.rendered_rgb();
            if header(&frame) == (top, bottom, left, right) {
                matched.push((at, differing_pixels(&frame, &reference_9100)));
            }
        }
        // A picture is two ticks wide, so the header sits on its rows for a pair of ticks.
        // The release's build draws it to the pixel — the whole frame, not only the band.
        assert_eq!(matched, [(183, 0), (184, 0)], "the ticks the header matched on");
        let start = 9100 - matched[0].0;
        assert_eq!(start, RELEASE_FIRST);
        assert!((REFERENCE_FIRST..=REFERENCE_LAST).contains(&start));
        assert_eq!(start - PORT_FIRST, 82, "the port's chain is 82 frames early (A53)");
    }

    #[test]
    fn the_port_draws_the_references_frames_at_the_references_own_ticks() {
        // At the release's own start every keyframe is 0 of 256,000 pixels; rediff measures
        // them at the port's chain, 82 frames earlier (A11).
        for index in KEYFRAMES {
            assert_eq!(differing_pixels(&at_release_index(index), &reference("endscrl", index, "rgb")), 0, "frame {index}");
        }
    }

    #[test]
    fn a_control_line_draws_nothing_and_lasts_its_own_count() {
        let (_, mut part) = started();
        let text = part.text[..5_599].to_vec();
        let lines = text_lines_of(&text);
        // The four forms in the text, and what `0x1662-0x1681` makes of each once the CRs are
        // gone: `[8`'s third byte is the newline, so it lasts 8*10 + '\n'-'0' = 42 rows.
        let mut forms: Vec<(&[u8], usize)> = Vec::new();
        for line in lines.iter().filter(|line| line.first() == Some(&CONTROL)) {
            match forms.iter_mut().find(|(form, _)| form == line) {
                Some((_, count)) => *count += 1,
                None => forms.push((line, 1)),
            }
        }
        forms.sort();
        assert_eq!(
            forms,
            [(&b"[10"[..], 14), (&b"[12"[..], 135), (&b"[15"[..], 8), (&b"[8"[..], 6)]
        );
        for (form, rows) in [(&b"[8\n"[..], 42), (b"[10\n", 10), (b"[12\n", 12), (b"[15\n", 15)] {
            assert_eq!(line_rows(form), rows, "{}", String::from_utf8_lossy(form));
        }
        assert_eq!(line_rows(b"Canada\n"), 25);

        // "Canada", then `[15`: the line under it in the text is the control line.
        assert_eq!(lines[17], b"Canada");
        assert_eq!(lines[18], b"[15");
        let offset: usize = lines[..18].iter().map(|line| line.len() + 1).sum();
        part.tptr = offset;
        part.read_line();
        assert_eq!(part.chars, 0, "`chars = 0` at 0x13ca");
        assert_eq!(&part.textline[..4], b"[15\n");
        assert_eq!(line_rows(&part.textline), 15);
        for row in 0..FONT_ROWS as i32 {
            part.line = row;
            assert!(part.render_line().iter().all(|p| *p == 0), "`[15` drew on row {row}");
        }
        // And the test can tell: the tree's rule, which draws the line, inks "15" — the
        // digits are in the order — on the very rows the reference leaves black.
        part.chars = 3;
        let inked = (0..FONT_ROWS as i32).any(|row| {
            part.line = row;
            part.render_line().iter().any(|p| *p != 0)
        });
        assert!(inked, "`[15` drawn as text would ink");

        // Run through the real loop: from "Canada"'s first row, the next line is read after
        // 25 rows of it and 15 of `[15`, and nothing is drawn in those 15.
        let (mut rig, mut part) = started();
        part.tptr = lines[..17].iter().map(|line| line.len() + 1).sum();
        run_to(&mut rig, &mut part, FIRST_SCROLL_TICK);
        let mut drawn = Vec::new();
        for _ in 0..(25 + 15 + 1) {
            let yscrl = part.yscrl;
            run_ticks(&mut rig, &mut part, 2);
            let row = &rig.surface.pixels()[yscrl * LINE_LEN..][..LINE_LEN];
            drawn.push(row.iter().any(|p| *p != 0));
        }
        assert!(drawn[..25].iter().any(|d| *d), "Canada");
        assert!(drawn[25..40].iter().all(|d| !*d), "the `[15` rows are blank");
        assert_eq!(&part.textline[..14], b"Epic MegaGames", "read on row 41");
    }

    /// The part's tick on which `do_scroll` reads the `%` line: ring row 11,483.
    const END_TICK: u32 = 22_967;

    #[test]
    fn the_part_ends_by_itself_on_the_percent_line() {
        // The text's line 537 is `%`, and the rows before it — 25 a line, a control line's own
        // count — sum to 11,482, so the `do_scroll` that reads it is the 11,483rd, on tick
        // 3 + 2 * 11,482 = 22,967. That call draws, sets the start and calls `exit(0)`.
        let (_, part) = started();
        let text = part.text[..5_599].to_vec();
        let lines = text_lines_of(&text);
        let end = lines.iter().position(|line| line.first() == Some(&END)).unwrap();
        assert_eq!((end, lines[end]), (537, &b"%"[..]));
        let rows: i32 = lines[..end]
            .iter()
            .map(|line| {
                let mut textline = line.to_vec();
                textline.extend_from_slice(&[LINE_END, 0]);
                line_rows(&textline)
            })
            .sum();
        assert_eq!(rows, 11_482);
        assert_eq!(FIRST_SCROLL_TICK + 2 * rows as u32, END_TICK);

        let (mut rig, mut part) = started();
        let mut last_ink = 0;
        loop {
            let at = part.tick;
            let status = part.frame(&mut rig.ctx(at));
            if rig.surface.pixels().iter().any(|p| *p != 0) {
                last_ink = at;
            }
            if status == PartStatus::Finished {
                assert_eq!(at, END_TICK, "the tick the part ended on");
                break;
            }
            assert!(at < END_TICK, "the part ran past its `%`");
        }
        assert_eq!(part.yscrl, 11_483 % RING_LINES, "the `%` call's line was written");
        assert_eq!(part.frame(&mut rig.ctx(END_TICK + 1)), PartStatus::Finished);
        // The screen it ends on is black: the text has scrolled off, the last ink leaving the
        // ring on tick 22,350 — the release's 8917 + 22,351 = 31,268, before the black frames
        // an unfiled capture of the original holds at 31,500 and 31,750
        // (`endscrl.md` §5).
        assert_eq!(last_ink, 22_350);
        assert!(rig.rendered_rgb().iter().all(|b| *b == 0));
        assert_eq!(RELEASE_FIRST + END_TICK, 31_884, "the release's `%`, by its own start");
        assert_eq!(PORT_FIRST + END_TICK, 31_802, "the port's, in a block 5 run");
    }

    #[test]
    #[cfg_attr(
        debug_assertions,
        ignore = "23,000 composed 640x400 ticks: about two minutes in debug; run with --release"
    )]
    fn a_demo_run_ends_after_the_scroller_ends_itself() {
        // The shell's end after a part's own `Finished` is the one it plays after ESC (D41):
        // the 64-tick fade over the last picture, 20 ticks of text mode, the end. A run of
        // the real part alone (no anchor: its ticks are the run's indices) shows it.
        use crate::shell::{Ending, RunSettings, Sequence, EXIT_BLANK_TICKS, EXIT_FADE_TICKS};
        let mut run = Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            crate::shell::start_view(),
            vec!["endscrl"],
            |_| Some(Box::new(Endscrl::new()) as Box<dyn Part>),
            None,
            RunSettings { ending: Ending::Exit, ..RunSettings::default() },
        )
        .unwrap();
        while !run.holding() {
            run.tick();
            assert!(run.index() <= END_TICK + 2, "the scroller did not end");
        }
        // The tick that read `%` was the part's last; the hold began on the next.
        assert_eq!(run.index(), END_TICK + 2);
        while !run.ended() {
            run.tick();
        }
        assert_eq!(run.index(), END_TICK + 2 + EXIT_FADE_TICKS + EXIT_BLANK_TICKS - 1);
    }
}
