//! ALKU — the intro text and the horizon pan.
//!
//! `ALKU/MAIN.C` (542 lines) is the part, over `ALKU/ASMYT.ASM` (the two
//! blitters) and `ALKU/COPPER.ASM` (the three copper routines). Its acts, in
//! order:
//!
//! 1. `:53-56` — mode 3, `init()`: `dis_partstart`, `tw_opengraph`
//!    (`ALKU/TWEAK.ASM:18-54`), `init_copper` (`COPPER.ASM:42-62`),
//!    `tw_setpalette(fade1)` — black — and the 640×200 `hzpic` blitted to the
//!    video memory's rows 50-349 (`MAIN.C:161-167`).
//! 2. `:59-64`, `:66-70`, `:72-77` — three title cards, each gated on
//!    `dis_sync()` **1**, **2** and **3**, drawn with `prtc` and faded in over
//!    64 frames, held for 300 and faded out over 64.
//! 3. `:79-85` — the fourth sync, the picture's 128-frame fade-in
//!    (`cop_dofade=128`) while `do_scroll(2)` steps the window.
//! 4. `:87-146` — the scroll loop: `do_scroll(1)` per outer iteration (a step
//!    every 9 copper frames, `#define SCRLF 9`), five text batches laid over
//!    the picture, each gated on `dis_sync() < 4+tptr`.
//! 5. `:147-152` — the exit: the final fade, `fonapois`, `close_copper`.
//!
//! # The display: 320×400 from a 640×200 picture
//!
//! `tw_opengraph` leaves the VGA in 320×400 with a **176-byte** line offset
//! (`ALKU/TWEAK.ASM:46`, CRTC 0x13 = `0x5813`; `reference-indices.md` §1.6).
//! A plane byte holds four pixels (`tw_putpixel`, `TWEAK.ASM:87-113`: plane
//! `x & 3`, byte `y*176 + x/4`), so one displayed line is 176 bytes = 704
//! pixels. The `outline` blitter (`ASMYT.ASM:60-108`) writes a source dword —
//! four source columns — into the four planes at byte column `a` of **two**
//! displayed rows (`di + ccc*352` and `+176`), and `init` calls it twice, at
//! byte column `a` and `a+88`. The second call is not a copy of the first for
//! long: `do_scroll`'s refresh (`MAIN.C:405-409`) rewrites byte column
//! `a/4+86` (and `+88`) from the source dword at `(a/4+86)*4`, which for byte
//! columns past 87 is the picture's *continuation* — columns 352 and up —
//! rather than a repeat. The two 352-pixel copies the init leaves are
//! therefore a starting state, and the refresh slides the second along behind
//! the first.
//!
//! The port keeps the video memory as the source's own flat byte array: a
//! plane byte at address `A` is pixel `4*A + plane`, so the surface's first
//! 350 lines × 704 pixels are the video memory and the `View` reads them with
//! `y_line_offset = 704`. The displayed window is `cop_start = a/4 + p*88`
//! bytes in with `cop_scrl = (a&3)*2` of fine panning, which the port writes
//! as `start_addr = 4*cop_start` and `pan = cop_scrl/2`. The fresh copy and
//! the refreshed continuation agree, so the two `p` values show the same
//! picture; the blit test pins the initial copies and the refresh test pins
//! the continuation the spill puts in the next row.
//!
//! **The view does not double-scan**, even though the picture is 200 rows
//! shown as 400: the source has two *memory* rows per picture row (`outline`
//! writes `di+ccc*352` and `+176`) and the refresh's `+88` write crosses into
//! the next row, which a folded pair cannot carry. The port keeps 400 rows of
//! 176 bytes (350 written, the rest zero) and a 704-pixel stride.
//!
//! # The copper, and the port's slots
//!
//! `init_copper` registers `copper1` in the DIS's slot **1** (the CRTC start
//! and pan, `COPPER.ASM:64-81`), `copper2` in slot **2** (the frame counter
//! and the palette upload, `:83-104`) and `copper3` in slot **0** (the 8.8
//! fade into `fadepal`, `:115-145`). The engine has the same three slots in
//! the same order — 0 at the top of the screen, 1 before retrace, 2 in
//! retrace — so the mapping is the identity and each routine keeps its
//! original slot:
//!
//! ```text
//! engine slot 0  <- copper3  the fade applier (after the display start)
//! engine slot 1  <- copper1  the CRTC start/pan writer (before retrace)
//! engine slot 2  <- copper2  frame_count++ and the palette upload (retrace)
//! ```
//!
//! Slot 0's fade is uploaded by slot 2 in the same tick, so a frame is
//! composed with the previous tick's upload — the original's line-20 write
//! and retrace upload, seen one frame later. A test drives all three slots.
//!
//! # The tick is the copper's frame
//!
//! The original's `frame_count` is incremented once per vertical blank by
//! `copper2`. The port's engine calls one tick per frame, so **one tick is one
//! copper frame**: `copper2`'s slot increments the same counter once a tick,
//! and `do_scroll`'s `while(frame_count<9);` becomes a 9-tick cadence — a
//! blocking `do_scroll(1)`/`(2)` is a tick-consuming wait that retries next
//! tick, never a spin. The counter is the one `dofade` and `wait(t)` pace
//! themselves with too, so their resets are the source's.
//!
//! # The stages and the preserved bugs
//!
//! - The card gates are `dis_sync() < 1`, `< 2`, `< 3` and `< 4`; the text
//!   batches wait `dis_sync() < 4+tptr` **with** `a&1` (`MAIN.C:138`), and
//!   `tptr` is post-incremented by the `switch (tptr++)`, so batch 0 waits for
//!   code **5**, not 4. The off-by-one is preserved and tested.
//! - The palette write-order aliasing (`MAIN.C:212`): after `palette2` and
//!   `fade2` are built, the byte copy `palette[192..768] = palette[..576]`
//!   repeats entries 0-63 three times, so every picture entry 64-255 is
//!   entry `v mod 64`. That is what makes the text XOR invisible over the
//!   picture (its values keep the low six bits).
//! - The refresh reads the source dword at `(a/4+86)*4`; the last refresh the
//!   scroll reaches is `a=316`'s, whose dword is columns 660-663 of a
//!   640-column row. The last byte, 663, is past the picture's row and comes
//!   from the *next* one; the port reads the flat picture the same way.
//! - `memset(tbuf,0,186*320)` clears the first 59,520 bytes of a 65,472-byte
//!   array, so the last 16 rows of `tbuf` keep the previous batch.
//! - The final `dofade(palette2,palette)` (`MAIN.C:147-149`) only touches
//!   entries 64-255, which the picture never indexes; it is kept as written.
//!
//! # The assets, and the three palette entries the release ships
//!
//! `hzpic` is the linked `HOI.IN0+IN1`, which differs from `HOI.U` in palette
//! entries 1-3 (`asset-inventory.md` §E.18): shipped `(10,10,10)`,
//! `(30,30,30)`, `(63,63,63)` against `HOI.U`'s `(2,2,3)`, `(5,5,7)`,
//! `(6,6,9)`. The port reads `HOI.U` — the picture bytes are identical — and
//! overrides those three entries with the shipped values, which is D24's
//! adopt-the-release rule for a data constant whose every other difference is
//! accounted for: with `HOI.U`'s own entries the reference's 1719 differs in
//! 1,595 pixels, 2719 in 4,171 and 5859 in 2,706; with the shipped entries
//! 1719 and 2719 are byte-identical and 5859's residual is the text-batch
//! phase (2,358 in this file's script, §A20). The measurement is an ignored
//! test in this file.
//!
//! `font` is `FONA.UH[783:]`, one byte earlier than the decoder's pixel area
//! at 784 (`asset-inventory.md` §8, A9). The port builds
//! `sr_assets::font::Font` from the decoded strip with a 20×30 cell — the
//! routine reads 32 rows of a 30-row strip, and the two rows past the file
//! are the linker's uninitialised padding; A9's resolution stands. A test
//! shows the one-byte shift moves no glyph the cards use.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use sr_assets::font::Font;
use sr_assets::{load_path, Image};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::View;
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "alku";

// ------------------------------------------------------------- the screen --

/// `MAIN.C:53`'s mode 3 and `TWEAK.ASM`'s tweak mode: 320 displayed pixels.
const VIEW_WIDTH: u16 = 320;
/// `TWEAK.ASM:42-47`: CRTC 9 = 0 and the byte-mode CRTC give 400 lines.
const VIEW_HEIGHT: u16 = 400;
/// `TWEAK.ASM:46` CRTC 0x13 = `0x5813`: 88 units × 2 = 176 bytes a line.
const LINE_BYTES: usize = 176;
/// Four pixels a plane byte (`TWEAK.ASM:87-113`).
const PIXELS_PER_BYTE: usize = 4;
/// One displayed line in the port's pixel units: 176 × 4.
const LINE_PIXELS: usize = LINE_BYTES * PIXELS_PER_BYTE;
/// The displayed rows the part writes: rows 50-349. Rows 350-399 stay zero
/// and the `View` wraps them into the zero rows above 50 (see [`Alku::prepare`]).
const VIDEO_ROWS: usize = 350;
/// The video memory the part addresses, in pixels.
const VIDEO_PIXELS: usize = VIDEO_ROWS * LINE_PIXELS;
/// `MAIN.C:165-166`: the picture's band begins at displayed row 50.
const PICTURE_FIRST_ROW: usize = 50;
/// The source rows `outline` blits: `MAIN.C:165-167`'s two 75-row halves.
const PICTURE_ROWS: usize = 150;

// ------------------------------------------------------------- the assets --

/// `MAIN.C:161` `memcpy(palette,hzpic+16,768)`: the picture and its palette.
const HZPIC_PATH: &str = "assets/ALKU/HOI.U";
const HZPIC_WIDTH: usize = 640;
const HZPIC_HEIGHT: usize = 200;
/// The linked `hzpic`'s palette entries 1-3 (`asset-inventory.md` §E.18).
const SHIPPED_PALETTE_ENTRIES: [(u8, [u8; 3]); 3] = [
    (1, [10, 10, 10]),
    (2, [30, 30, 30]),
    (3, [63, 63, 63]),
];

/// `ALKU/INCLUDE.ASM:17`: `FONA.UH` (1500×30) as the font strip.
const FONT_PATH: &str = "assets/ALKU/FONA.UH";
/// `MAIN.C:234`: `fonap[32]=1500-20`, so the blank is the strip's last 20
/// columns and the strip is a whole number of 20-column cells.
const FONT_CELL: (u16, u16) = (20, 30);
const FONT_FIRST: char = 'A';
/// `MAIN.C:218`/`:225` scan 32 rows; the strip has 30 and A9 says the last two
/// are linker padding. The port draws the 30 the file holds.
const FONT_ROWS: usize = 30;
/// `MAIN.C:40`'s `fonaorder`, the strip's 76 ink runs in order.
const FONT_ORDER: &[u8] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:\x8f\x8f()+-*='\x8f\x99";
/// `MAIN.C:235`: `fonaw[32]=16`.
const BLANK_WIDTH: u8 = 16;
/// The cell has no glyph: the source's `fonaw` is 0 for these and the pen
/// advances two pixels.
const NO_CELL: u8 = u8::MAX;

// -------------------------------------------------------------- the scroll --

/// `MAIN.C:7` `#define SCRLF 9`.
const SCRLF: u16 = 9;
/// `MAIN.C:87` `a<320`; the inner wait's own bound is `a<319` (`:138`).
const SCROLL_END: u16 = 320;
/// `MAIN.C:47` `int a=0,p=0,tptr=0` — the picture phase starts them at 1,1.
const SCROLL_A_START: u16 = 1;
const SCROLL_P_START: u16 = 1;
/// `MAIN.C:83` `cop_dofade=128`: the picture's fade-in.
const PICTURE_FADE_STEPS: i32 = 128;
/// `MAIN.C:91`/`:96`: the text fades.
const TEXT_FADE_STEPS: i32 = 64;
/// `MAIN.C:64`: 64 fade steps each way.
const CARD_FADE_STEPS: u32 = 64;
/// `MAIN.C:64` `wait(300)`.
const CARD_HOLD_TICKS: u32 = 300;
/// `MAIN.C:148` `f>63/SCRLF` — `63/9 = 7`.
const FINAL_FADE_THRESHOLD: i32 = 7;
/// `MAIN.C:141` `if(aa<320-12) fmaketext(aa+16)`.
const MAKETEXT_LIMIT: u16 = 320 - 12;
const MAKETEXT_LEAD: u16 = 16;

/// `MAIN.C:101` `memset(tbuf,0,186*320)` on the 186×352 array.
const TBUF_ROWS: usize = 186;
const TBUF_STRIDE: usize = 352;
const TBUF_LEN: usize = TBUF_ROWS * TBUF_STRIDE;
const TBUF_CLEAR: usize = TBUF_ROWS * 320;
/// `MAIN.C:352`/`:361`: the text's rows 1..183 and its 100-row base.
const TEXT_FIRST_ROW: usize = 1;
const TEXT_ROWS: usize = 183;
const TEXT_BASE_ROW: usize = 100;

/// `MAIN.C:257-268` / `:471-495`: the two `fonapois` masks.
const FONAPOIS_FIRST: usize = 160 * 64;
const FONAPOIS_END: usize = 160 * (64 + 256);
const FFONAPOIS_FIRST: usize = 40 * 64;
const FFONAPOIS_END: usize = 40 * (64 + 256 + 10);
const FADE_MASK: u8 = 0x3F;

// -------------------------------------------------------------- the music --

/// `MAIN.C:59`/`:66`/`:72`/`:79`/`:138`: the four card/picture gates and the
/// text batches' base code.
const FIRST_GATE: i32 = 1;
const PICTURE_GATE: i32 = 4;

// ------------------------------------------------------------- the copper --

/// `COPPER.ASM:110` `fadepal db 768*2 dup(?)`: the high byte of an 8.8 value
/// a palette entry, the low byte its fraction.
const FADEPAL_LEN: usize = 1536;

/// A palette the source mutates byte by byte, as `array[0..768] of byte` does.
type Rgb = [u8; 3];
type Palette768 = [Rgb; 256];

/// The font strip as the routine indexes it: the cells the scan found, keyed
/// by the part's character order, and their widths.
struct Strip {
    font: Font,
    /// `fonap[ch]`'s cell index, or [`NO_CELL`].
    cell: [u8; 256],
    /// `fonaw[ch]`.
    width: [u8; 256],
}

impl Strip {
    /// `MAIN.C:214-235`'s scan: the strip's ink runs, in the part's order.
    fn scan(font: Font) -> Result<Strip, String> {
        let mut cell = [NO_CELL; 256];
        let mut width = [0u8; 256];
        for (index, &ch) in FONT_ORDER.iter().enumerate() {
            let glyph = font
                .glyph(cell_char(index))
                .ok_or_else(|| format!("the strip has no cell {index} for {ch:#04x}"))?;
            width[usize::from(ch)] = (glyph.len() / FONT_ROWS) as u8;
            cell[usize::from(ch)] = index as u8;
        }
        width[usize::from(b' ')] = BLANK_WIDTH;
        Ok(Strip { font, cell, width })
    }

    /// The glyph for `ch`: `FONT_ROWS` rows of `width(ch)` columns, the
    /// strip's own 2-bit values.
    fn glyph(&self, ch: u8) -> Option<&[u8]> {
        let cell = self.cell[usize::from(ch)];
        if cell == NO_CELL {
            return None;
        }
        self.font.glyph(cell_char(usize::from(cell)))
    }

    fn width(&self, ch: u8) -> usize {
        usize::from(self.width[usize::from(ch)])
    }

    /// `MAIN.C:173-181`: a strip value 1/2/3 becomes 0x40/0x80/0xC0.
    fn ink(&self, ch: u8, column: usize, row: usize) -> u8 {
        let Some(glyph) = self.glyph(ch) else {
            return 0;
        };
        let width = self.width(ch);
        if column >= width || row >= FONT_ROWS {
            return 0;
        }
        match glyph[row * width + column] & 3 {
            1 => 0x40,
            2 => 0x80,
            3 => 0xC0,
            _ => 0,
        }
    }
}

/// `MAIN.C:259`'s mapping from an order position to a cell: the first cell's
/// character plus the index — the form `Font::glyph` takes.
fn cell_char(index: usize) -> char {
    char::from_u32(FONT_FIRST as u32 + index as u32).expect("a cell index is a character")
}

/// `COPPER.ASM:67-72`: the CRTC start address, in plane bytes.
fn cop_start(a: u16, p: u16) -> u16 {
    a / 4 + p * 88
}

/// `COPPER.ASM:74-78`: the AC pel-panning value, `(a&3)*2`.
fn cop_scrl(a: u16) -> u8 {
    ((a & 3) * 2) as u8
}

/// Write `cop_start`/`cop_scrl` into the view the way `copper1` writes the
/// CRTC: the start is a plane-byte address (four pixels each) and the AC
/// register's unit is half a pixel (`docs/fidelity/cred.md` §3, `View::pan`).
fn apply_view(view: &mut View, start: u16, scrl: u8) {
    view.start_addr = u32::from(start) * PIXELS_PER_BYTE as u32;
    view.x_offset = 0;
    view.pan = scrl / 2;
}

/// The three copper routines' shared state.
struct CopperState {
    /// `COPPER.ASM:10` `frame_count`.
    frame_count: u16,
    /// `:15-16` `cop_start`/`cop_scrl`.
    cop_start: u16,
    cop_scrl: u8,
    /// `:13-14` `cop_pal`/`do_pal`.
    cop_pal: [u8; 768],
    do_pal: bool,
    /// `:106-113` `cop_dofade`, `fadepal` and `cop_fadepal`'s 768 deltas.
    cop_dofade: i32,
    fadepal: [u8; FADEPAL_LEN],
    fade: [i32; 768],
}

impl CopperState {
    fn new() -> CopperState {
        CopperState {
            frame_count: 0,
            cop_start: 0,
            cop_scrl: 0,
            cop_pal: [0; 768],
            do_pal: false,
            cop_dofade: 0,
            fadepal: [0; FADEPAL_LEN],
            fade: [0; 768],
        }
    }

    /// `COPPER.ASM:115-145`: the fade applier. The source sets `cop_pal` to a
    /// **pointer** to `fadepal` and then adds in place; `copper2` uploads it at
    /// the retrace, so what reaches the DAC is the *post-add* palette. The port
    /// copies `fadepal` after the add, which is the same bytes at the same
    /// upload — and the last step's accumulation is uploaded, as the source's
    /// is, rather than left one step behind.
    fn slot0(&mut self, _ctx: &mut PartCtx) {
        if self.cop_dofade == 0 {
            return;
        }
        self.cop_dofade -= 1;
        for index in 0..768 {
            let low = u16::from(self.fadepal[768 + index]);
            let high = u16::from(self.fadepal[index]);
            let delta = self.fade[index] as i16 as u16;
            let sum = ((high << 8) | low).wrapping_add(delta);
            self.fadepal[index] = (sum >> 8) as u8;
            self.fadepal[768 + index] = sum as u8;
        }
        self.cop_pal.copy_from_slice(&self.fadepal[..768]);
        self.do_pal = true;
    }

    /// `COPPER.ASM:64-81`: the CRTC start and pan.
    fn slot1(&mut self, ctx: &mut PartCtx) {
        apply_view(ctx.view, self.cop_start, self.cop_scrl);
    }

    /// `COPPER.ASM:83-104`: the frame counter and the palette upload.
    fn slot2(&mut self, ctx: &mut PartCtx) {
        self.frame_count = self.frame_count.wrapping_add(1);
        if self.do_pal {
            let mut palette = Palette::new();
            for index in 0..256 {
                palette.set_entry(
                    index as u8,
                    [
                        self.cop_pal[index * 3],
                        self.cop_pal[index * 3 + 1],
                        self.cop_pal[index * 3 + 2],
                    ],
                );
            }
            ctx.set_palette(palette);
            self.do_pal = false;
        }
    }
}

/// Where the part is. The card's number is its index in [`CARDS`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Stage {
    /// `MAIN.C:59`/`:66`/`:72`/`:79`: waiting for `dis_sync()` to reach `code`.
    Sync { code: i32, card: usize },
    /// `MAIN.C:64`/`:70`/`:77`: the card's fade in, hold and out.
    Card { card: usize, phase: CardPhase },
    /// `MAIN.C:84-85`: the 128-frame fade-in's `do_scroll(2)`.
    PictureFade,
    /// `MAIN.C:87-146`: the scroll loop.
    Main(MainPhase),
    /// `MAIN.C:147-149`: the final `dofade(palette2,palette)`, 64 steps.
    FinalFade { a: u32 },
    /// The part has ended and keeps its last frame.
    Done,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CardPhase {
    FadeIn { a: u32 },
    Hold { ticks: u32 },
    FadeOut { a: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MainPhase {
    /// The `if(f==0) … else if(f==50) … else if(f>50 && cop_dofade==0) …`
    /// chain (`MAIN.C:89-144`).
    Branch,
    /// The batch's `while(((a&1) || dis_sync()<4+tptr) …)` (`:138`).
    Inner { code: i32 },
    /// `fmaketext`'s `while(a<=scrl) do_scroll(0);` (`:468`).
    Tail { scrl: u16 },
    /// The outer loop's blocking `do_scroll(1)` (`:145`).
    Outer,
}

/// Which of the three int[768] tables the fade applier adds this phase.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum FadeSource {
    PicIn,
    TextIn,
    TextOut,
}

/// The three cards, `MAIN.C:61-63`, `:68-69`, `:74-76`.
const CARDS: &[&[(u16, &[u8])]] = &[
    &[(120, b"A"), (160, b"Future Crew"), (200, b"Production")],
    &[(160, b"First Presented"), (200, b"at Assembly 93")],
    &[(120, b"in"), (160, b"\x8f"), (179, b"\x99")],
];

/// ALKU: the picture, the font, the scroll and the copper.
pub struct Alku {
    shared: Rc<RefCell<CopperState>>,
    picture: Vec<u8>,
    strip: Option<Strip>,
    /// `MAIN.C:30-31` `palette`/`palette2`, as triples (the source's bytes).
    palette: Palette768,
    palette2: Palette768,
    /// `:33-34` `fade1` (black) and `fade2` (the text ramp).
    fade1: Palette768,
    fade2: Palette768,
    /// `:36-38` the fade tables the copper applies.
    picin: [i32; 768],
    textin: [i32; 768],
    textout: [i32; 768],
    /// `:45` `tbuf[186][352]` and `:44` `dtau[30000]`.
    tbuf: Vec<u8>,
    dtau: Vec<i32>,
    /// `:47` `a`, `p` and `tptr`.
    a: u16,
    p: u16,
    tptr: usize,
    /// `:87-146`'s `f`.
    f: i32,
    /// Where the part is.
    stage: Stage,
    /// The part's own tick.
    tick: u32,
    hzpic_path: &'static str,
    font_path: &'static str,
    /// Whether the three linked-hzpic entries are applied (D24). `true` for
    /// the part; `false` only for the counterfactual measurement test.
    shipped_palette: bool,
}

impl Alku {
    /// A part with nothing loaded: `init` reads the two assets.
    pub fn new() -> Alku {
        Alku {
            shared: Rc::new(RefCell::new(CopperState::new())),
            picture: Vec::new(),
            strip: None,
            palette: [[0; 3]; 256],
            palette2: [[0; 3]; 256],
            fade1: [[0; 3]; 256],
            fade2: [[0; 3]; 256],
            picin: [0; 768],
            textin: [0; 768],
            textout: [0; 768],
            tbuf: vec![0; TBUF_LEN],
            dtau: Vec::new(),
            a: 0,
            p: 0,
            tptr: 0,
            f: 60,
            stage: Stage::Sync { code: FIRST_GATE, card: 0 },
            tick: 0,
            hzpic_path: HZPIC_PATH,
            font_path: FONT_PATH,
            shipped_palette: true,
        }
    }

    /// [`new`](Alku::new) reading its assets from elsewhere.
    #[cfg(test)]
    fn at(hzpic: &'static str, font: &'static str) -> Alku {
        Alku { hzpic_path: hzpic, font_path: font, ..Alku::new() }
    }

    /// [`at`](Alku::at) with `HOI.U`'s own entries 1-3, for the counterfactual.
    #[cfg(test)]
    fn at_curated(hzpic: &'static str, font: &'static str) -> Alku {
        Alku {
            hzpic_path: hzpic,
            font_path: font,
            shipped_palette: false,
            ..Alku::new()
        }
    }

    /// [`load_path`] with the part's own error, so every asset failure names
    /// the path the same way.
    fn read_image(&self, path: &str) -> Result<Image, PartError> {
        load_path(Path::new(path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{path}: {error}"),
        })
    }

    /// The mode set's display state (`TWEAK.ASM:18-54`): 320×400, a 704-pixel
    /// line, origin 0, no split.
    fn set_display(ctx: &mut PartCtx) {
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(LINE_PIXELS as u16);
        ctx.view.set_origin(0);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
    }

    /// `MAIN.C:161-167`: `outline` twice for every byte column 0-87, which
    /// fills rows 50-349 with the picture's first 352 columns twice.
    fn blit_picture(&self, video: &mut [u8]) {
        for column in 0..88usize {
            for source_row in 0..PICTURE_ROWS {
                let source = source_row * HZPIC_WIDTH + column * 4;
                for &col in &[column, column + 88] {
                    // `outline`'s two clears, at the base destination's rows
                    // 48 and 49 (`di-352`, `di-176`).
                    for row in [PICTURE_FIRST_ROW - 2, PICTURE_FIRST_ROW - 1] {
                        for plane in 0..PIXELS_PER_BYTE {
                            let at = (row * LINE_BYTES + col) * PIXELS_PER_BYTE + plane;
                            if at < video.len() {
                                video[at] = 0;
                            }
                        }
                    }
                    for row in [
                        PICTURE_FIRST_ROW + 2 * source_row,
                        PICTURE_FIRST_ROW + 2 * source_row + 1,
                    ] {
                        for plane in 0..PIXELS_PER_BYTE {
                            let at = (row * LINE_BYTES + col) * PIXELS_PER_BYTE + plane;
                            if let Some(value) = self.picture.get(source + plane) {
                                if at < video.len() {
                                    video[at] = *value;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// `MAIN.C:405-409`: refresh the byte column the window is about to reach.
    /// The source dword is read out of the flat picture, so the last columns
    /// of a row read the next row's first bytes — the preserved bug.
    fn refresh(&self, video: &mut [u8], a: u16) {
        let column = usize::from(a / 4) + 86;
        for source_row in 0..PICTURE_ROWS {
            let source = source_row * HZPIC_WIDTH + column * 4;
            for &col in &[column, column + 88] {
                for row in [PICTURE_FIRST_ROW - 2, PICTURE_FIRST_ROW - 1] {
                    for plane in 0..PIXELS_PER_BYTE {
                        let at = (row * LINE_BYTES + col) * PIXELS_PER_BYTE + plane;
                        if at < video.len() {
                            video[at] = 0;
                        }
                    }
                }
                for row in [
                    PICTURE_FIRST_ROW + 2 * source_row,
                    PICTURE_FIRST_ROW + 2 * source_row + 1,
                ] {
                    for plane in 0..PIXELS_PER_BYTE {
                        let at = (row * LINE_BYTES + col) * PIXELS_PER_BYTE + plane;
                        let value = self.picture.get(source + plane).copied().unwrap_or(0);
                        if at < video.len() {
                            video[at] = value;
                        }
                    }
                }
            }
        }
    }

    /// `MAIN.C:272-291` `prt`: OR a glyph into the video memory at (`x`, `y`).
    fn prt(&self, video: &mut [u8], x: usize, y: usize, text: &[u8]) {
        let Some(strip) = self.strip.as_ref() else {
            return;
        };
        let mut pen = x;
        for &ch in text {
            let width = strip.width(ch);
            for column in 0..width {
                for row in 0..32usize {
                    // Rows 30 and 31 are the linker's padding (A9); the loop
                    // keeps the source's bound and inks nothing there.
                    let ink = strip.ink(ch, column, row);
                    let at = (y + row) * LINE_PIXELS + pen + column;
                    if ink != 0 && at < video.len() {
                        video[at] |= ink;
                    }
                }
            }
            pen += width + 2;
        }
    }

    /// `MAIN.C:293-299` `prtc`: centre the string about `x`.
    fn prtc(&self, video: &mut [u8], x: usize, y: usize, text: &[u8]) {
        let Some(strip) = self.strip.as_ref() else {
            return;
        };
        let mut width = 0usize;
        for &ch in text {
            width += strip.width(ch) + 2;
        }
        let left = x.saturating_sub(width / 2);
        self.prt(video, left, y, text);
    }

    /// `MAIN.C:324-340` `addtext`: write a string into `tbuf`.
    fn addtext(&mut self, tx: usize, ty: usize, text: &[u8]) {
        self.text_into_tbuf(tx, ty, text);
    }

    /// `MAIN.C:417-434` `faddtext`: the same, one `do_scroll(0)` per character.
    fn faddtext(&mut self, video: &mut [u8], tx: usize, ty: usize, text: &[u8]) {
        let Some(widths) = self.strip.as_ref().map(|strip| {
            text.iter().map(|&ch| strip.width(ch)).collect::<Vec<_>>()
        }) else {
            return;
        };
        let width = widths.iter().map(|width| width + 2).sum::<usize>() / 2;
        let mut pen = tx;
        for (&ch, &cell_width) in text.iter().zip(&widths) {
            {
                let strip = self.strip.as_ref().expect("the strip was read above");
                for column in 0..cell_width {
                    for row in 0..32usize {
                        let ink = strip.ink(ch, column, row);
                        let at = (ty + row) * TBUF_STRIDE + pen + column;
                        let at = at as isize - width as isize;
                        if at >= 0 {
                            if let Some(slot) = self.tbuf.get_mut(at as usize) {
                                *slot = ink;
                            }
                        }
                    }
                }
            }
            self.do_scroll0(video);
            pen += cell_width + 2;
        }
    }

    /// The body of `addtext`.
    fn text_into_tbuf(&mut self, tx: usize, ty: usize, text: &[u8]) {
        let Some(strip) = self.strip.as_ref() else {
            return;
        };
        let mut width = 0usize;
        for &ch in text {
            width += strip.width(ch) + 2;
        }
        let width = width / 2;
        let mut pen = tx;
        for &ch in text {
            let cell_width = strip.width(ch);
            for column in 0..cell_width {
                for row in 0..32usize {
                    let ink = strip.ink(ch, column, row);
                    let at = (ty + row) * TBUF_STRIDE + pen + column;
                    let at = at as isize - width as isize;
                    if at >= 0 {
                        if let Some(slot) = self.tbuf.get_mut(at as usize) {
                            *slot = ink;
                        }
                    }
                }
            }
            pen += cell_width + 2;
        }
    }

    fn tbuf_at(&self, row: usize, column: isize) -> u8 {
        let at = row as isize * TBUF_STRIDE as isize + column;
        if at < 0 {
            0
        } else {
            self.tbuf.get(at as usize).copied().unwrap_or(0)
        }
    }

    /// `MAIN.C:436-469` `fmaketext`, without its trailing scroll wait (which
    /// is [`MainPhase::Tail`]).
    fn fmaketext(&mut self, video: &mut [u8], scrl: u16) {
        self.dtau.clear();
        for phase in 0..4usize {
            let mut x = phase;
            while x < 320 {
                for y in TEXT_FIRST_ROW..TEXT_FIRST_ROW + TEXT_ROWS {
                    let now = self.tbuf_at(y, x as isize);
                    let before = self.tbuf_at(y, x as isize - 2);
                    if now != before {
                        let address =
                            (x / 4 + y * LINE_BYTES + TEXT_BASE_ROW * LINE_BYTES) as i32;
                        self.dtau.push(address);
                        self.dtau.push(i32::from(now ^ before));
                    }
                }
                self.do_scroll0(video);
                x += 4;
            }
            self.dtau.push(-1);
            self.dtau.push(-1);
        }

        // `MAIN.C:456-466`: the direct XOR, one `do_scroll(0)` per column.
        for x in 0..320usize {
            let pix = x + usize::from(scrl);
            let plane = pix & 3;
            let column = pix / 4;
            for y in TEXT_FIRST_ROW..TEXT_FIRST_ROW + TEXT_ROWS {
                let address = y * LINE_BYTES + TEXT_BASE_ROW * LINE_BYTES + column;
                let left = self.tbuf_at(y, x as isize - 2);
                let right = self.tbuf_at(y, x as isize - 1);
                xor_pixel(video, address, plane, left);
                xor_pixel(video, address + 88, plane, right);
            }
            self.do_scroll0(video);
        }
    }

    /// `ASMYT.ASM:12-54` `ascrolltext`: move the text with the window by the
    /// differences `fmaketext` recorded.
    fn ascrolltext(&mut self, video: &mut [u8], scrl: u16) {
        if self.dtau.len() < 8 {
            return;
        }
        let scrl = usize::from(scrl);
        let mut at = 0usize;
        for cx in 0..4usize {
            let plane = (cx + scrl) & 3;
            let offset = (cx + scrl) / 4;
            loop {
                let address = self.dtau[at];
                let value = self.dtau[at + 1];
                at += 2;
                if address == -1 {
                    break;
                }
                xor_pixel(video, address as usize + offset, plane, value as u8);
            }
        }
    }

    /// `MAIN.C:250-270` / `:471-495` `fonapois`: clear the top two bits of
    /// the addressed bytes, a plane at a time.
    fn fonapois(&self, video: &mut [u8]) {
        for plane in 0..PIXELS_PER_BYTE {
            for address in FONAPOIS_FIRST..FONAPOIS_END {
                let at = address * PIXELS_PER_BYTE + plane;
                if at < video.len() {
                    video[at] &= FADE_MASK;
                }
            }
        }
    }

    /// `MAIN.C:471-495` `ffonapois`: the `long far *` form, so the address
    /// range steps in four-byte groups.
    fn ffonapois(&mut self, video: &mut [u8]) {
        for plane in 0..PIXELS_PER_BYTE {
            for address in FFONAPOIS_FIRST * 4..FFONAPOIS_END * 4 {
                let at = address * PIXELS_PER_BYTE + plane;
                if at < video.len() {
                    video[at] &= FADE_MASK;
                }
            }
            self.do_scroll0(video);
        }
    }

    /// `MAIN.C:397-412`'s non-blocking half: one step if nine frames are up.
    fn do_scroll0(&mut self, video: &mut [u8]) {
        let count = self.shared.borrow().frame_count;
        if count >= SCRLF {
            self.shared.borrow_mut().frame_count = count - SCRLF;
            self.scroll_step(video, 0);
        }
    }

    /// The blocking half: `false` when the tick was spent waiting, `true` when
    /// a step happened.
    fn blocking_scroll(&mut self, video: &mut [u8], mode: u8) -> bool {
        let count = self.shared.borrow().frame_count;
        if count < SCRLF {
            return false;
        }
        self.shared.borrow_mut().frame_count = count - SCRLF;
        self.scroll_step(video, mode);
        true
    }

    /// `MAIN.C:402-411`: one step of the window.
    fn scroll_step(&mut self, video: &mut [u8], mode: u8) {
        if mode == 1 {
            self.ascrolltext(video, self.a + self.p * 352);
        }
        let start = cop_start(self.a, self.p);
        let scrl = cop_scrl(self.a);
        {
            let mut shared = self.shared.borrow_mut();
            shared.cop_start = start;
            shared.cop_scrl = scrl;
        }
        if self.a & 3 == 0 {
            self.refresh(video, self.a);
        }
        self.a += 1;
        self.p ^= 1;
    }

    fn set_fade_source(&self, source: FadeSource) {
        let table: &[i32; 768] = match source {
            FadeSource::PicIn => &self.picin,
            FadeSource::TextIn => &self.textin,
            FadeSource::TextOut => &self.textout,
        };
        self.shared.borrow_mut().fade.copy_from_slice(table);
    }

    /// `dofade`'s step `a`: the source's `(pal1*(64-a)+pal2*a)>>6`.
    fn dofade_step(&self, pal1: &Palette768, pal2: &Palette768, a: u32) {
        let mut bytes = [0u8; 768];
        for entry in 0..256 {
            for channel in 0..3 {
                bytes[entry * 3 + channel] = ((u32::from(pal1[entry][channel]) * (64 - a)
                    + u32::from(pal2[entry][channel]) * a)
                    >> 6) as u8;
            }
        }
        let mut shared = self.shared.borrow_mut();
        shared.cop_pal = bytes;
        shared.do_pal = true;
    }

    /// `MAIN.C:79-85`: the fourth gate's entry point.
    fn begin_picture(&mut self) {
        self.a = SCROLL_A_START;
        self.p = SCROLL_P_START;
        self.f = 60;
        self.tptr = 0;
        self.dtau.clear();
        // `MAIN.C:82`: `cop_fadepal=picin`.
        self.set_fade_source(FadeSource::PicIn);
        let mut shared = self.shared.borrow_mut();
        // `MAIN.C:81`: `memcpy(fadepal,fade1,768)` — the high half zero; the
        // low half is BSS zero and the picture fade is its first user.
        shared.fadepal[..768].fill(0);
        shared.cop_dofade = PICTURE_FADE_STEPS;
        shared.frame_count = 0;
    }

    /// `MAIN.C:89-144`'s `switch (tptr++)`, with the case value already taken.
    fn draw_batch(&mut self, video: &mut [u8], case: usize) {
        match case {
            0 => {
                self.addtext(160, 50, b"Graphics");
                self.addtext(160, 90, b"Marvel");
                self.addtext(160, 130, b"Pixel");
                self.ffonapois(video);
            }
            1 => {
                self.faddtext(video, 160, 50, b"Music");
                self.faddtext(video, 160, 90, b"Purple Motion");
                self.faddtext(video, 160, 130, b"Skaven");
                self.ffonapois(video);
            }
            2 => {
                self.faddtext(video, 160, 30, b"Code");
                self.faddtext(video, 160, 70, b"Psi");
                self.faddtext(video, 160, 110, b"Trug");
                self.faddtext(video, 160, 148, b"Wildfire");
                self.ffonapois(video);
            }
            3 => {
                self.faddtext(video, 160, 50, b"Additional Design");
                self.faddtext(video, 160, 90, b"Abyss");
                self.faddtext(video, 160, 130, b"Gore");
                self.ffonapois(video);
            }
            4 => self.ffonapois(video),
            _ => {
                self.faddtext(video, 160, 80, b"BUG BUG BUG");
                self.faddtext(video, 160, 130, b"Timing error");
                self.ffonapois(video);
            }
        }
    }

    /// One tick. `code` is this tick's `dis_sync()` answer, read once.
    fn tick_stage(&mut self, video: &mut [u8], code: i32, _tick: u32) -> PartStatus {
        loop {
            match self.stage {
                Stage::Done => return PartStatus::Finished,
                Stage::Sync { code: gate, card } => {
                    if code < gate {
                        return PartStatus::Continuing;
                    }
                    if gate < PICTURE_GATE {
                        for &(y, text) in CARDS[card] {
                            self.prtc(video, 160, usize::from(y), text);
                        }
                        self.dofade_step(&self.fade1, &self.fade2, 0);
                        self.shared.borrow_mut().frame_count = 0;
                        self.stage = Stage::Card { card, phase: CardPhase::FadeIn { a: 1 } };
                        return PartStatus::Continuing;
                    }
                    self.begin_picture();
                    self.stage = Stage::PictureFade;
                }
                Stage::Card { card, phase } => match phase {
                    CardPhase::FadeIn { a } => {
                        if a < CARD_FADE_STEPS {
                            self.dofade_step(&self.fade1, &self.fade2, a);
                            self.shared.borrow_mut().frame_count = 0;
                            self.stage = Stage::Card {
                                card,
                                phase: if a + 1 == CARD_FADE_STEPS {
                                    CardPhase::Hold { ticks: 0 }
                                } else {
                                    CardPhase::FadeIn { a: a + 1 }
                                },
                            };
                            return PartStatus::Continuing;
                        }
                    }
                    CardPhase::Hold { ticks } => {
                        // `wait(300)` is 300 frames: the first hold tick is
                        // the one after the fade-in's last step, and the
                        // fade-out begins the tick after the 300th.
                        if ticks < CARD_HOLD_TICKS {
                            self.stage =
                                Stage::Card { card, phase: CardPhase::Hold { ticks: ticks + 1 } };
                            return PartStatus::Continuing;
                        }
                        self.shared.borrow_mut().frame_count = 0;
                        self.dofade_step(&self.fade2, &self.fade1, 0);
                        self.stage = Stage::Card { card, phase: CardPhase::FadeOut { a: 1 } };
                        return PartStatus::Continuing;
                    }
                    CardPhase::FadeOut { a } => {
                        if a < CARD_FADE_STEPS {
                            self.dofade_step(&self.fade2, &self.fade1, a);
                            self.shared.borrow_mut().frame_count = 0;
                            if a + 1 == CARD_FADE_STEPS {
                                self.fonapois(video);
                                self.stage = if card + 1 < CARDS.len() {
                                    Stage::Sync {
                                        code: FIRST_GATE + card as i32 + 1,
                                        card: card + 1,
                                    }
                                } else {
                                    Stage::Sync { code: PICTURE_GATE, card: CARDS.len() }
                                };
                            } else {
                                self.stage =
                                    Stage::Card { card, phase: CardPhase::FadeOut { a: a + 1 } };
                            }
                            return PartStatus::Continuing;
                        }
                    }
                },
                Stage::PictureFade => {
                    if !self.blocking_scroll(video, 2) {
                        return PartStatus::Continuing;
                    }
                    if self.shared.borrow().cop_dofade == 0 {
                        self.stage = Stage::Main(MainPhase::Branch);
                    } else {
                        return PartStatus::Continuing;
                    }
                }
                Stage::FinalFade { a } => {
                    if a < CARD_FADE_STEPS {
                        self.dofade_step(&self.palette2, &self.palette, a);
                        self.shared.borrow_mut().frame_count = 0;
                        self.stage = Stage::FinalFade { a: a + 1 };
                        return PartStatus::Continuing;
                    }
                    // `MAIN.C:150`: the final `fonapois()` after the fade.
                    self.fonapois(video);
                    self.stage = Stage::Done;
                    return PartStatus::Finished;
                }
                Stage::Main(_) => return self.main_tick(video, code),
            }
        }
    }

    /// `MAIN.C:87-146`: the scroll loop, one tick at a time.
    fn main_tick(&mut self, video: &mut [u8], code: i32) -> PartStatus {
        loop {
            match self.stage {
                Stage::Main(MainPhase::Branch) => {
                    if self.f == 0 {
                        self.set_fade_source(FadeSource::TextIn);
                        self.shared.borrow_mut().cop_dofade = TEXT_FADE_STEPS;
                        self.f += 20;
                        self.stage = Stage::Main(MainPhase::Outer);
                    } else if self.f == 50 {
                        self.set_fade_source(FadeSource::TextOut);
                        self.shared.borrow_mut().cop_dofade = TEXT_FADE_STEPS;
                        self.f += 1;
                        self.stage = Stage::Main(MainPhase::Outer);
                    } else if self.f > 50 && self.shared.borrow().cop_dofade == 0 {
                        let palette = self.palette;
                        {
                            let mut shared = self.shared.borrow_mut();
                            for (entry, rgb) in palette.iter().enumerate() {
                                shared.cop_pal[entry * 3..entry * 3 + 3]
                                    .copy_from_slice(rgb);
                            }
                            shared.do_pal = true;
                        }
                        self.f += 1;
                        self.tbuf[..TBUF_CLEAR].fill(0);
                        let case = self.tptr;
                        self.tptr += 1;
                        self.draw_batch(video, case);
                        self.stage = Stage::Main(MainPhase::Inner { code: 4 + self.tptr as i32 });
                    } else {
                        self.f += 1;
                        self.stage = Stage::Main(MainPhase::Outer);
                    }
                }
                Stage::Main(MainPhase::Inner { code: threshold }) => {
                    let waiting = ((self.a & 1) != 0 || code < threshold) && self.a < 319;
                    if waiting {
                        self.do_scroll0(video);
                        return PartStatus::Continuing;
                    }
                    self.f = 0;
                    let aa = self.a;
                    if aa < MAKETEXT_LIMIT {
                        let scrl = aa + MAKETEXT_LEAD;
                        self.fmaketext(video, scrl);
                        self.stage = Stage::Main(MainPhase::Tail { scrl });
                    } else {
                        self.stage = Stage::Main(MainPhase::Outer);
                    }
                }
                Stage::Main(MainPhase::Tail { scrl }) => {
                    if self.a <= scrl {
                        self.do_scroll0(video);
                        return PartStatus::Continuing;
                    }
                    self.stage = Stage::Main(MainPhase::Outer);
                }
                Stage::Main(MainPhase::Outer) => {
                    if !self.blocking_scroll(video, 1) {
                        return PartStatus::Continuing;
                    }
                    if self.a >= SCROLL_END {
                        // `MAIN.C:147-149`: the final fade only when `f>7`,
                        // and its first step is this tick's (the rest are the
                        // `Stage::FinalFade` arm above).
                        if self.f > FINAL_FADE_THRESHOLD {
                            self.dofade_step(&self.palette2, &self.palette, 0);
                            self.shared.borrow_mut().frame_count = 0;
                            self.stage = Stage::FinalFade { a: 1 };
                            return PartStatus::Continuing;
                        }
                        // `MAIN.C:150` runs `fonapois()` on this path too.
                        self.fonapois(video);
                        self.stage = Stage::Done;
                        return PartStatus::Finished;
                    }
                    self.stage = Stage::Main(MainPhase::Branch);
                }
                _ => return PartStatus::Finished,
            }
        }
    }

    /// Read the assets and prepare the screen; see the module docs.
    fn prepare(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        if ctx.music().is_none() {
            return Err(PartError {
                part: NAME,
                message: "this part waits on the music's start codes (ALKU/MAIN.C:59, :66, :72, \
                          :79, :138) and the run has no music to wait on"
                    .into(),
            });
        }
        let picture = self.read_image(self.hzpic_path)?;
        if (picture.width() as usize, picture.height() as usize)
            != (HZPIC_WIDTH, HZPIC_HEIGHT)
        {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} is {}x{}; the part reads {HZPIC_WIDTH}x{HZPIC_HEIGHT}",
                    self.hzpic_path,
                    picture.width(),
                    picture.height()
                ),
            });
        }
        self.picture = picture.pixels().to_vec();

        let mut bytes = [0u8; 768];
        for entry in 0..256 {
            let rgb = picture.palette.entry(entry as u8);
            bytes[entry * 3..entry * 3 + 3].copy_from_slice(&rgb);
        }
        // D24: the linked hzpic's three entries, read out of HOI.U's palette
        // and replaced with the shipped bytes (`asset-inventory.md` §E.18).
        if self.shipped_palette {
            for &(entry, rgb) in &SHIPPED_PALETTE_ENTRIES {
                bytes[usize::from(entry) * 3..usize::from(entry) * 3 + 3].copy_from_slice(&rgb);
            }
        }
        self.build_ramps(&bytes);
        // `MAIN.C:212`: the byte copy that repeats entries 0-63.
        for at in 192..768 {
            bytes[at] = bytes[at - 192];
        }
        for entry in 0..256 {
            self.palette[entry] = [bytes[entry * 3], bytes[entry * 3 + 1], bytes[entry * 3 + 2]];
        }
        // `MAIN.C:237-242`, after both the ramp and the alias.
        for (at, &byte) in bytes.iter().enumerate() {
            let palette2 = triple_byte(&self.palette2, at);
            self.textin[at] = (i32::from(palette2) - i32::from(byte)) * 256 / 64;
            self.textout[at] = (i32::from(byte) - i32::from(palette2)) * 256 / 64;
            self.picin[at] = i32::from(byte) * 256 / 128;
        }

        let strip_image = self.read_image(self.font_path)?;
        if (strip_image.width(), strip_image.height()) != (1500, FONT_ROWS as u16) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} is {}x{}; the strip is 1500x{FONT_ROWS}",
                    self.font_path,
                    strip_image.width(),
                    strip_image.height()
                ),
            });
        }
        let font = Font::from_image(strip_image, FONT_CELL, FONT_FIRST).map_err(|error| {
            PartError { part: NAME, message: format!("{}: {error}", self.font_path) }
        })?;
        self.strip = Some(Strip::scan(font).map_err(|message| PartError {
            part: NAME,
            message: format!("{}: {message}", self.font_path),
        })?);

        if ctx.surface.pixels().len() < VIDEO_PIXELS {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "the part addresses {VIDEO_PIXELS} pixels of video memory and the surface \
                     holds {}",
                    ctx.surface.pixels().len()
                ),
            });
        }

        // The mode set clears 256 KB (`TWEAK.ASM:29-34`); the picture lands in
        // rows 50-349 (`MAIN.C:165-167`).
        ctx.surface.clear(0);
        self.blit_picture(ctx.surface.pixels_mut());
        Alku::set_display(ctx);
        // `MAIN.C:160`: `tw_setpalette(fade1)`, black.
        ctx.set_palette(Palette::new());

        {
            let mut shared = self.shared.borrow_mut();
            *shared = CopperState::new();
        }
        let slot0 = Rc::clone(&self.shared);
        ctx.set_copper(0, Box::new(move |ctx: &mut PartCtx| slot0.borrow_mut().slot0(ctx)));
        let slot1 = Rc::clone(&self.shared);
        ctx.set_copper(1, Box::new(move |ctx: &mut PartCtx| slot1.borrow_mut().slot1(ctx)));
        let slot2 = Rc::clone(&self.shared);
        ctx.set_copper(2, Box::new(move |ctx: &mut PartCtx| slot2.borrow_mut().slot2(ctx)));

        self.tbuf.fill(0);
        self.dtau.clear();
        self.a = 0;
        self.p = 0;
        self.tptr = 0;
        self.f = 60;
        self.tick = 0;
        self.stage = Stage::Sync { code: FIRST_GATE, card: 0 };
        Ok(())
    }

    /// `MAIN.C:184-210`: `palette2` and `fade2` from the *unaliased* palette.
    fn build_ramps(&mut self, palette: &[u8; 768]) {
        let mut palette2 = [0u8; 768];
        let mut fade2 = [0u8; 768];
        for at in (0..768).step_by(3) {
            if at < 64 * 3 {
                palette2[at..at + 3].copy_from_slice(&palette[at..at + 3]);
                continue;
            }
            let base = if at < 128 * 3 {
                1
            } else if at < 192 * 3 {
                2
            } else {
                3
            };
            for channel in 0..3 {
                let value = palette[base * 3 + channel];
                fade2[at + channel] = value;
                palette2[at + channel] = ((u32::from(value) * 63
                    + u32::from(palette[at % (64 * 3) + channel]) * (63 - u32::from(value)))
                    >> 6) as u8;
            }
        }
        for entry in 0..256 {
            self.palette2[entry] = [
                palette2[entry * 3],
                palette2[entry * 3 + 1],
                palette2[entry * 3 + 2],
            ];
            self.fade2[entry] =
                [fade2[entry * 3], fade2[entry * 3 + 1], fade2[entry * 3 + 2]];
        }
    }
}

fn triple_byte(palette: &Palette768, at: usize) -> u8 {
    palette[at / 3][at % 3]
}

/// XOR one video pixel (`xor [es:bx+di],al`).
fn xor_pixel(video: &mut [u8], address: usize, plane: usize, value: u8) {
    let at = address * PIXELS_PER_BYTE + plane;
    if at < video.len() {
        video[at] ^= value;
    }
}

impl Part for Alku {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        self.prepare(ctx)
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        if ctx.exit_requested {
            self.stage = Stage::Done;
            return PartStatus::Finished;
        }
        let code = ctx.music().map_or(i32::MIN, |music| music.sync());
        let tick = self.tick;
        self.tick_stage(ctx.surface.pixels_mut(), code, tick)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{Music, MusicPosition};
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::{dac_expand, Surface};

    const TEST_HZPIC: &str = "../../assets/ALKU/HOI.U";
    const TEST_FONT: &str = "../../assets/ALKU/FONA.UH";

    /// The player a unit test hands the part: `sync()` answers a scripted code
    /// a tick, and every other call is the null reading.
    struct Player {
        codes: Vec<i32>,
        at: usize,
    }

    impl Player {
        fn scripted(codes: Vec<i32>) -> Player {
            Player { codes, at: 0 }
        }
    }

    impl Music for Player {
        fn poll(&mut self) {}
        fn position(&self) -> MusicPosition {
            MusicPosition::default()
        }
        fn musplus(&mut self) -> i32 {
            -32
        }
        fn musrow(&mut self) -> i32 {
            0
        }
        fn muscode(&mut self, _wanted: i32) -> i32 {
            0
        }
        fn sync(&mut self) -> i32 {
            self.codes.get(self.at).copied().unwrap_or_else(|| *self.codes.last().unwrap_or(&0))
        }
        fn getmframe(&mut self) -> i32 {
            0
        }
        fn setmframe(&mut self, _frame: i32) {}
    }

    /// The engine's pieces, so a `PartCtx` can be built for one call at a time.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
        player: Player,
        tick: u32,
    }

    impl Rig {
        fn new(player: Player) -> Rig {
            Rig {
                surface: Surface::new(640, 400),
                view: View::new(320, 200),
                palette: Palette::new(),
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
                player,
                tick: 0,
            }
        }

        fn ctx_at(&mut self, tick: u32, exit: bool) -> PartCtx<'_> {
            self.player.at = tick as usize;
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(&mut self.player),
                tick,
                0,
                1,
                true,
                exit,
                false,
            )
        }

        fn ctx(&mut self, tick: u32) -> PartCtx<'_> {
            self.ctx_at(tick, false)
        }

        /// One engine slot, taken out of the registry for the call and put back
        /// exactly as `Engine::run_copper` does.
        fn run_slot(&mut self, slot: usize) {
            let Some(mut callback) = self.copper.take(slot) else {
                return;
            };
            {
                let tick = self.tick;
                let mut ctx = self.ctx(tick);
                callback(&mut ctx);
            }
            self.copper.restore(slot, callback);
        }

        /// One tick in the engine's order: slot 0, the part, slot 1, slot 2.
        fn step(&mut self, part: &mut Alku) -> PartStatus {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx(tick));
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            status
        }

        fn step_exit(&mut self, part: &mut Alku) -> PartStatus {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx_at(tick, true));
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            status
        }

        /// The frame composed during the tick: the view and the palette as
        /// slots 1 and 2 leave them are the *next* frame's, so this is called
        /// from [`step_render`] before those slots run.
        fn rendered_rgb(&self) -> Vec<u8> {
            let mut out = Vec::with_capacity(usize::from(VIEW_WIDTH) * usize::from(VIEW_HEIGHT) * 3);
            for y in 0..VIEW_HEIGHT {
                for x in 0..VIEW_WIDTH {
                    let at = self.view.source_index(&self.surface, x, y);
                    let entry = self.palette.entry(self.surface.pixels()[at]);
                    out.push(dac_expand(entry[0]));
                    out.push(dac_expand(entry[1]));
                    out.push(dac_expand(entry[2]));
                }
            }
            out
        }

        fn step_render(&mut self, part: &mut Alku) -> (PartStatus, Vec<u8>) {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx(tick));
            let frame = self.rendered_rgb();
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            (status, frame)
        }
    }

    fn started_with(player: Player) -> (Rig, Alku) {
        let mut rig = Rig::new(player);
        let mut part = Alku::at(TEST_HZPIC, TEST_FONT);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Alku) {
        started_with(Player::scripted(Vec::new()))
    }

    /// A code script for `second1`'s transitions, one entry a tick.
    fn trace_script(last_tick: u32) -> Vec<i32> {
        trace_script_shifted(last_tick, 0)
    }

    /// The same transition frames moved by `shift` (positive is later).
    fn trace_script_shifted(last_tick: u32, shift: i32) -> Vec<i32> {
        // boot = 404 + tick; the trace's transition frames.
        const AT: [(i32, i32); 9] = [
            (1537, 1),
            (2097, 2),
            (2509, 3),
            (3069, 4),
            (3630, 5),
            (4189, 6),
            (4750, 7),
            (5310, 8),
            (5459, 9),
        ];
        (0..=last_tick)
            .map(|tick| {
                let boot = 404 + tick as i32;
                AT.iter()
                    .rev()
                    .find(|(at, _)| boot >= *at + shift)
                    .map_or(0, |(_, code)| *code)
            })
            .collect()
    }

    /// All four gates published as soon as they can be: card 1 at tick 0,
    /// card 2 at 428, card 3 at 856, the picture at 1284 — each card is the
    /// source's 64 + 300 + 64 ticks and the next gate is checked on the tick
    /// after the fade-out's last step.
    fn picture_script(last_tick: u32) -> Vec<i32> {
        (0..=last_tick)
            .map(|tick| match tick {
                0..=427 => 1,
                428..=855 => 2,
                856..=1283 => 3,
                _ => 4,
            })
            .collect()
    }

    fn run_to(rig: &mut Rig, part: &mut Alku, tick: u32) {
        while rig.tick < tick {
            rig.step(part);
        }
    }

    fn reference(index: u32) -> Vec<u8> {
        std::fs::read(format!("../../reference/parts/alku/frame_{index:06}.rgb")).unwrap()
    }

    /// **The lengths are asserted first**: a short read must not compare a
    /// prefix and report zero differences as if it had compared the frame.
    fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
        assert_eq!(
            rendered.len(),
            expected.len(),
            "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
            rendered.len(),
            expected.len()
        );
        rendered
            .chunks_exact(3)
            .zip(expected.chunks_exact(3))
            .filter(|(a, b)| a != b)
            .count()
    }

    // --- the brief's four tests ---------------------------------------------

    #[test]
    fn the_stage_gates_follow_the_music_and_a_skipped_code_still_passes_them() {
        // Code 1 arrives at tick 5; the script then jumps straight to 3, and
        // the card-2 gate must still pass on the tick it is reached.
        let mut script = vec![0; 600];
        for (tick, code) in script.iter_mut().enumerate() {
            *code = match tick {
                0..=4 => 0,
                5..=24 => 1,
                _ => 3,
            };
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        assert_eq!(part.stage, Stage::Sync { code: 1, card: 0 });
        run_to(&mut rig, &mut part, 6);
        assert!(
            matches!(part.stage, Stage::Card { card: 0, .. }),
            "card 1 must begin on the tick code 1 arrives, got {:?}",
            part.stage
        );
        // Card 1 is 64 fade-in ticks, 300 hold ticks and 64 fade-out ticks;
        // its 428th tick (tick 5 + 427) hands over to the code-2 gate's stage.
        run_to(&mut rig, &mut part, 5 + 428);
        assert!(
            matches!(part.stage, Stage::Sync { code: 2, card: 1 }),
            "card 1 must hand over to the code-2 gate, got {:?}",
            part.stage
        );
        // Code 3 is already published: the skipped code-2 gate passes at once.
        rig.step(&mut part);
        assert!(
            matches!(part.stage, Stage::Card { card: 1, .. }),
            "a skipped code must not block the next gate, got {:?}",
            part.stage
        );
    }

    #[test]
    fn the_scroll_advances_one_step_every_nine_ticks() {
        let (mut rig, mut part) = started_with(Player::scripted(picture_script(1500)));
        run_to(&mut rig, &mut part, 1285);
        assert_eq!(part.stage, Stage::PictureFade);
        for tick in 1285..=1292u32 {
            rig.step(&mut part);
            assert_eq!(
                (rig.view.start_addr, rig.view.pan),
                (0, 0),
                "tick {tick}: no step before the ninth"
            );
        }
        rig.step(&mut part);
        assert_eq!(
            (rig.view.start_addr, rig.view.pan),
            (4 * 88, 1),
            "the ninth tick is the first step, (a=1,p=1)"
        );
        assert_eq!((part.a, part.p), (2, 0), "the step increments a and flips p");
        for tick in 1294..=1301u32 {
            rig.step(&mut part);
            assert_eq!(
                (rig.view.start_addr, rig.view.pan),
                (4 * 88, 1),
                "tick {tick}: still the first step's view"
            );
        }
        rig.step(&mut part);
        assert_eq!(
            (rig.view.start_addr, rig.view.pan),
            (0, 2),
            "the second step is (a=2,p=0): byte 0, pan 2"
        );
    }

    #[test]
    fn the_pan_is_the_crtc_start_and_pel_panning() {
        for a in [0u16, 1, 2, 3, 4, 7, 308, 309, 318, 319] {
            for p in [0u16, 1] {
                let start = cop_start(a, p);
                assert_eq!(start, a / 4 + p * 88, "cop_start({a},{p})");
                assert_eq!(cop_scrl(a), ((a & 3) * 2) as u8, "cop_scrl({a})");
                let mut view = View::new(VIEW_WIDTH, VIEW_HEIGHT);
                apply_view(&mut view, start, cop_scrl(a));
                assert_eq!(view.start_addr, u32::from(start) * 4, "the start in pixels");
                assert_eq!(view.x_offset, 0, "the panning is the AC register's");
                assert_eq!(view.pan, (a & 3) as u8, "AC value / 2 pixels");
                assert_eq!(
                    view.source_index(&Surface::new(640, 400), 0, 0),
                    usize::from(a + p * 352),
                    "the first displayed pixel"
                );
            }
        }
    }

    #[test]
    fn the_font_strip_is_the_routines_own_scan_and_the_one_byte_shift_is_invisible() {
        let image = load_path(Path::new(TEST_FONT)).unwrap();
        assert_eq!((image.width(), image.height()), (1500, 30));
        let font = Font::from_image(image, FONT_CELL, FONT_FIRST).unwrap();
        let strip = Strip::scan(font).unwrap();
        let mut cells = 0;
        for index in 0..FONT_ORDER.len() {
            if strip.font.glyph(cell_char(index)).is_some() {
                cells += 1;
            }
        }
        assert_eq!(cells, 76, "the strip's ink runs, one per order entry");
        for (index, &ch) in FONT_ORDER.iter().enumerate() {
            let glyph = strip
                .glyph(ch)
                .unwrap_or_else(|| panic!("{ch:#x} (cell {index}) has no glyph"));
            assert_eq!(glyph.len() % FONT_ROWS, 0);
            assert_eq!(strip.width(ch), glyph.len() / FONT_ROWS);
        }
        assert_eq!(strip.width(b' '), usize::from(BLANK_WIDTH), "fonaw[32]=16");
        assert_eq!(strip.glyph(b' '), None, "the blank has no ink");
        // A9: the linked array is `FONA.UH[783:]` and the decoder's pixels
        // start at 784. Every run is the same glyph one column over.
        let bytes = std::fs::read("../../upstream/SecondReality/ALKU/FONA.UH").unwrap();
        let shifted = runs(&bytes[783..]);
        let unshifted = runs(&bytes[784..]);
        assert_eq!(shifted.len(), 76);
        assert_eq!(unshifted.len(), 76);
        for (index, (a, b)) in shifted.iter().zip(&unshifted).enumerate() {
            assert_eq!(a.1, b.1, "cell {index}'s width");
            assert_eq!(a.2, b.2, "cell {index}'s glyph bytes");
        }
        assert_eq!(bytes[783], 0, "the byte the shift adds is the palette's last");
    }

    /// The ink runs of a raw 1500×30 strip: `(start, width, glyph bytes)`.
    fn runs(pixels: &[u8]) -> Vec<(usize, usize, Vec<u8>)> {
        let mut out = Vec::new();
        let mut x = 0;
        while x < 1500 {
            if !(0..FONT_ROWS).any(|y| pixels[y * 1500 + x] != 0) {
                x += 1;
                continue;
            }
            let start = x;
            while x < 1500 && (0..FONT_ROWS).any(|y| pixels[y * 1500 + x] != 0) {
                x += 1;
            }
            let width = x - start;
            let mut glyph = Vec::with_capacity(width * FONT_ROWS);
            for y in 0..FONT_ROWS {
                glyph.extend_from_slice(&pixels[y * 1500 + start..y * 1500 + start + width]);
            }
            out.push((start, width, glyph));
        }
        out
    }

    // --- the assets and the preserved bugs ----------------------------------

    #[test]
    fn the_picture_is_hoi_u_with_the_three_entries_the_release_ships() {
        let image = load_path(Path::new(TEST_HZPIC)).unwrap();
        assert_eq!((image.width(), image.height()), (640, 200));
        // The curated entries, the divergence E.18 records.
        assert_eq!(image.palette.entry(1), [2, 2, 3]);
        assert_eq!(image.palette.entry(2), [5, 5, 7]);
        assert_eq!(image.palette.entry(3), [6, 6, 9]);
        let (_, part) = started();
        for &(entry, rgb) in &SHIPPED_PALETTE_ENTRIES {
            assert_eq!(
                part.palette[usize::from(entry)],
                rgb,
                "entry {entry} is the shipped hzpic's"
            );
        }
        // And every entry 64-255 is entry `v mod 64` (MAIN.C:212).
        for entry in 0..256 {
            assert_eq!(
                part.palette[entry],
                part.palette[entry % 64],
                "entry {entry} must alias entry {}",
                entry % 64
            );
        }
    }

    #[test]
    fn the_picture_blit_writes_the_band_and_the_refresh_reads_past_the_row() {
        let (rig, part) = started();
        // Row 50 is source row 0's first 352 columns, twice.
        for column in 0..88usize {
            for plane in 0..4usize {
                let source = column * 4 + plane;
                let first = (50 * LINE_BYTES + column) * 4 + plane;
                let second = (50 * LINE_BYTES + column + 88) * 4 + plane;
                assert_eq!(rig.surface.pixels()[first], part.picture[source]);
                assert_eq!(rig.surface.pixels()[second], part.picture[source]);
            }
        }
        // Above the band stays zero.
        assert!(rig.surface.pixels()[..49 * LINE_PIXELS].iter().all(|&v| v == 0));
        // At a=316 the refresh's column is 165 and its source dword starts at
        // column 660: past the 640-column row, so it reads row 0's columns
        // 20-23 from the flat picture.
        let mut video = rig.surface.pixels().to_vec();
        part.refresh(&mut video, 316);
        let column = 165usize;
        for plane in 0..4usize {
            let at = (50 * LINE_BYTES + column) * 4 + plane;
            assert_eq!(
                video[at],
                part.picture[660 + plane],
                "the refresh's dword crosses into the next picture row"
            );
            assert_eq!(
                part.picture[660 + plane],
                part.picture[640 + 20 + plane],
                "column 660 of a 640-wide picture is the next row's column 20"
            );
        }
    }

    #[test]
    fn the_tbuf_clear_keeps_the_arrays_last_sixteen_rows() {
        // `MAIN.C:101` clears 186*320 bytes of a 186*352 array.
        assert_eq!(TBUF_CLEAR, 186 * 320);
        assert_eq!(TBUF_LEN, 186 * 352);
        // Drive the part's own batch branch (`MAIN.C:99-101`): fill the buffer
        // with a marker just before the tick that clears it, and read what the
        // production clear left.
        let (mut rig, mut part) = started_with(Player::scripted(picture_script(1500)));
        for _ in 0..1600 {
            // The picture fade's last tick falls through into the branch in the
            // same call, so the marker goes in on every tick before it.
            if matches!(part.stage, Stage::PictureFade | Stage::Main(MainPhase::Branch)) {
                part.tbuf.fill(7);
            }
            rig.step(&mut part);
            if matches!(part.stage, Stage::Main(MainPhase::Inner { .. })) {
                break;
            }
        }
        assert!(
            matches!(part.stage, Stage::Main(MainPhase::Inner { .. })),
            "the first batch ran"
        );
        assert!(
            part.tbuf[..TBUF_CLEAR].iter().all(|byte| *byte != 7),
            "the first {TBUF_CLEAR} bytes were cleared (and the batch's text is not the marker)"
        );
        // The batch's own text is drawn after the clear and may land in the
        // last rows' first 320 columns, so read the columns it never writes
        // (320 and up) of the rows the clear never reaches (170..185).
        for row in 170..TBUF_ROWS {
            assert!(
                part.tbuf[row * TBUF_STRIDE + 320..(row + 1) * TBUF_STRIDE]
                    .iter()
                    .all(|byte| *byte == 7),
                "row {row}'s columns 320.. survive the clear"
            );
        }
        assert_eq!(part.tbuf[TBUF_LEN - 1], 7, "and the array's last byte");
    }

    #[test]
    fn the_text_operations_keep_the_pictures_low_six_bits() {
        // Both the text XOR (0x40/0x80/0xC0) and the two masks (&0x3f) are
        // invisible under the aliased palette: `palette[v] == palette[v%64]`.
        for value in 0u8..=255 {
            for ink in [0x00u8, 0x40, 0x80, 0xC0] {
                assert_eq!((value ^ ink) % 64, value % 64);
            }
            assert_eq!((value & FADE_MASK) % 64, value % 64);
        }
    }

    #[test]
    fn the_fade_applier_uploads_the_post_add_palette() {
        // `COPPER.ASM:122-124` sets `cop_pal` to a *pointer* to `fadepal` and
        // `copper2` uploads it at the retrace, after the add: the port copies
        // `fadepal` after the add. A delta that crosses a high-byte boundary
        // distinguishes the two.
        let mut state = CopperState::new();
        state.cop_dofade = 1;
        state.fade[0] = 100; // 2.200 + 100 = 2.300: the high byte moves
        state.fade[1] = -240; // and a negative delta
        state.fadepal[0] = 2; // high
        state.fadepal[768] = 200; // low
        state.fadepal[1] = 60;
        state.fadepal[769] = 0;
        let mut rig = Rig::new(Player::scripted(Vec::new()));
        let mut ctx = rig.ctx(0);
        state.slot0(&mut ctx);
        // 2*256 + 200 + 100 = 812 = 0x032C; 60*256 - 240 = 15120 = 0x3B10.
        assert_eq!((state.fadepal[0], state.fadepal[768]), (3, 44));
        assert_eq!((state.fadepal[1], state.fadepal[769]), (0x3B, 0x10));
        assert_eq!(state.cop_dofade, 0);
        assert!(state.do_pal);
        assert_eq!(
            &state.cop_pal[..3],
            &[3, 0x3B, 0],
            "cop_pal is fadepal after the add, so the upload is the post-add step"
        );
    }

    // --- the music's gates --------------------------------------------------

    #[test]
    fn the_picture_fade_hands_over_to_a_batch_gated_on_code_five() {
        let (mut rig, mut part) = started_with(Player::scripted(picture_script(1500)));
        // The picture gate is tick 1284; the fade loop's first step is 1293
        // and its 15th is 1419, where `cop_dofade` has reached zero.
        run_to(&mut rig, &mut part, 1420);
        assert_eq!(
            part.stage,
            Stage::Main(MainPhase::Inner { code: 5 }),
            "tptr is post-incremented, so batch 0 waits for code 5"
        );
    }

    #[test]
    fn all_three_copper_slots_are_installed_and_each_does_its_job() {
        let (mut rig, mut part) = started_with(Player::scripted(picture_script(1500)));
        for slot in 0..3 {
            let set = rig.ctx(0).copper_is_set(slot);
            assert!(set, "slot {slot} is registered");
        }
        run_to(&mut rig, &mut part, 1285);
        assert_eq!(part.stage, Stage::PictureFade);
        // Slot 0 (the fade applier) decrements `cop_dofade` once a tick.
        let dofade = part.shared.borrow().cop_dofade;
        let frame_count = part.shared.borrow().frame_count;
        rig.step(&mut part); // tick 1285
        assert_eq!(part.shared.borrow().cop_dofade, dofade - 1, "slot 0's fade");
        // Slot 2 (the frame counter) increments once a tick.
        assert_eq!(part.shared.borrow().frame_count, frame_count + 1, "slot 2's counter");
        // Slot 1 (the CRTC writer) has not run a step yet; at the ninth tick
        // after the reset it writes the first step's start and pan.
        assert_eq!(rig.view.start_addr, 0, "no step before the ninth tick");
        for _ in 0..8 {
            rig.step(&mut part); // ticks 1286..1293
        }
        assert_eq!(
            (rig.view.start_addr, rig.view.pan),
            (4 * 88, 1),
            "slot 1 writes (a=1,p=1)'s start and pan"
        );
    }

    #[test]
    fn an_exit_request_ends_every_wait_on_the_tick_it_is_seen() {
        let (mut rig, mut part) = started_with(Player::scripted(vec![0; 10]));
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.stage, Stage::Done);
    }

    // --- the reference frames ----------------------------------------------

    /// The reference frame at boot `boot` is the tick `boot - 404`'s composed
    /// frame, which is what [`Rig::step_render`] captures.
    fn render_at(boot: u32) -> Vec<u8> {
        let tick = boot - 404;
        let (mut rig, mut part) = started_with(Player::scripted(trace_script(tick)));
        while rig.tick < tick {
            rig.step(&mut part);
        }
        let (_, frame) = rig.step_render(&mut part);
        frame
    }

    #[test]
    fn the_three_held_keyframes_match_the_reference_rgb() {
        for boot in [900u32, 1719, 2719] {
            let frame = render_at(boot);
            assert_eq!(
                differing_pixels(&frame, &reference(boot)),
                0,
                "boot {boot}: the held frame is not the reference's"
            );
        }
    }

    #[test]
    fn the_scrolling_keyframes_picture_is_the_references_under_its_batch_phase() {
        // The reference's frame is bare: either its session reached the empty
        // text batch's `fmaketext` (`MAIN.C:141`, `aa<308`), which rebuilds
        // `dtau` from the zeroed `tbuf` and erases the old text, or its fade
        // was elsewhere. The port's trace-faithful phase reaches the batch at
        // `aa=308` and skips it, so its text survives into the next fade.
        // **With the post-gate transitions moved by -9 frames — or by +10,
        // which `the_5859_phase_sensitivity` also finds — the frame is
        // byte-identical.** That is what makes the pixel machinery, the
        // palette and the pan exact and the phase the only difference.
        let tick = 5859 - 404;
        let (mut rig, mut part) =
            started_with(Player::scripted(trace_script_after_gate(tick, -9)));
        while rig.tick < tick {
            rig.step(&mut part);
        }
        let (_, frame) = rig.step_render(&mut part);
        assert_eq!(
            differing_pixels(&frame, &reference(5859)),
            0,
            "boot 5859 with the batch transitions moved"
        );
    }

    #[test]
    fn the_final_fade_runs_sixty_four_ticks_when_the_loop_leaves_f_above_seven() {
        let tick = 6000;
        let (mut rig, mut part) = started_with(Player::scripted(trace_script(tick)));
        while !matches!(part.stage, Stage::FinalFade { .. }) && rig.tick <= tick {
            rig.step(&mut part);
        }
        assert!(
            matches!(part.stage, Stage::FinalFade { .. }),
            "the scroll loop leaves f above 7, so the final fade runs; got {:?} at f={}",
            part.stage,
            part.f
        );
        let mut fade_ticks = 0u32;
        let mut status = PartStatus::Continuing;
        while part.stage != Stage::Done {
            if matches!(part.stage, Stage::FinalFade { .. }) {
                fade_ticks += 1;
            }
            status = rig.step(&mut part);
            assert!(fade_ticks <= 64, "the fade ran past 64 ticks");
        }
        assert_eq!(fade_ticks, 64, "the final fade is the source's 64 steps");
        assert_eq!(status, PartStatus::Finished, "and then the part retires");
    }

    #[test]
    fn the_scrolling_keyframes_port_phase_differs_only_in_the_text_xor_pixels() {
        let key = render_at(5859);
        let differing = differing_pixels(&key, &reference(5859));
        assert_eq!(differing, 2358, "the measured residual of the port's own phase");
        // Every differing pixel is inside the old text's glyph region (rows
        // 150-243, columns 40-279); the picture band and the black margins are
        // byte-identical.
        let expected = reference(5859);
        for y in 0..400u32 {
            for x in 0..320u32 {
                let at = ((y * 320 + x) * 3) as usize;
                if key[at..at + 3] != expected[at..at + 3] {
                    assert!(
                        (150..=243).contains(&y) && (40..=279).contains(&x),
                        "({x},{y}) differs outside the text's glyph region"
                    );
                }
            }
        }
        // D19's support rule: the frame moves, so K-1 and K+1 are different
        // pictures. The reference's own sessions differ by a whole pan step.
        assert_ne!(render_at(5858), key, "5858 is a different pan");
        assert_ne!(render_at(5860), key, "5860 is a different pan");
    }

    /// The D24 measurement, ignored: with the curated `HOI.U` entries the
    /// reference's cards and its picture band differ; with the shipped
    /// entries they do not. Run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_curated_palette_entries_leave_the_reference_different() {
        let mut rig = Rig::new(Player::scripted(trace_script(5455)));
        let mut part = Alku::at_curated(TEST_HZPIC, TEST_FONT);
        part.init(&mut rig.ctx(0)).unwrap();
        let mut frames = Vec::new();
        for tick in 0..=5455u32 {
            if [496u32, 1315, 2315, 5455].contains(&tick) {
                let (_, frame) = rig.step_render(&mut part);
                frames.push((404 + tick, frame));
            } else {
                rig.step(&mut part);
            }
        }
        let mut measured = Vec::new();
        for (boot, frame) in frames {
            let expected = reference(boot);
            let differing = differing_pixels(&frame, &expected);
            println!(
                "boot {boot}: {differing} of 128000 pixels differ with the curated entries"
            );
            measured.push((boot, differing));
        }
        // A20's numbers, pinned: a silent change here would let the note's
        // counterfactual go stale.
        assert_eq!(measured, [(900, 0), (1719, 1595), (2719, 4171), (5859, 2706)]);
    }

    /// The text batches' transitions moved by `shift` while the four gates stay
    /// where the trace has them: the original's CPU-time shift, simulated.
    fn trace_script_after_gate(last_tick: u32, shift: i32) -> Vec<i32> {
        const AT: [(i32, i32); 9] = [
            (1537, 1),
            (2097, 2),
            (2509, 3),
            (3069, 4),
            (3630, 5),
            (4189, 6),
            (4750, 7),
            (5310, 8),
            (5459, 9),
        ];
        (0..=last_tick)
            .map(|tick| {
                let boot = 404 + tick as i32;
                AT.iter()
                    .rev()
                    .find(|(at, code)| boot >= *at + if *code >= 5 { shift } else { 0 })
                    .map_or(0, |(_, code)| *code)
            })
            .collect()
    }

    /// The phase sensitivity of 5859 to the music's transition frames, ignored.
    /// Run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_5859_phase_sensitivity() {
        let mut at_zero = None;
        let mut at_minus_nine = None;
        let mut at_plus_ten = None;
        for shift in -18..=18 {
            let tick = 5859 - 404;
            let (mut rig, mut part) =
                started_with(Player::scripted(trace_script_after_gate(tick, shift)));
            while rig.tick < tick {
                rig.step(&mut part);
            }
            let (_, frame) = rig.step_render(&mut part);
            let expected = reference(5859);
            let differing = differing_pixels(&frame, &expected);
            println!(
                "shift {shift:+3}: {differing:5} pixels differ; a={} p={} f={} stage={:?}",
                part.a, part.p, part.f, part.stage
            );
            match shift {
                0 => at_zero = Some(differing),
                -9 => at_minus_nine = Some(differing),
                10 => at_plus_ten = Some(differing),
                _ => {}
            }
        }
        // A20's three measured rows, pinned.
        assert_eq!(at_minus_nine, Some(0), "the reference phase's reproduction");
        assert_eq!(at_plus_ten, Some(0), "a second phase that reproduces it");
        assert_eq!(at_zero, Some(2358), "the port's own phase");
    }
}
