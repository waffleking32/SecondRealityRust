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
//! # The release's build (D49)
//!
//! The port runs the shipped `MAIN/DATA/ALKU.EXE`'s loop, read from its unpacked image
//! (`sr_assets::exe::unpack`; the offsets below are the image's, and
//! `the_release_loop_and_its_closing_routine_are_the_images` pins their bytes). It differs
//! from the tree's `MAIN.C` in five places:
//!
//! - the loop runs while `a <= 320` (`0x1380`), and `do_scroll` returns at once past 320
//!   (`0x2315`);
//! - `fmaketext` runs only when `aa < 304` (`0x134f`), not 308;
//! - every pass ends `do_scroll(tptr<5 ? 1 : 0)` (`0x136c`): once the fifth batch (the
//!   empty `case 4`) has cleared the text, the window no longer carries it, and the pass
//!   no longer waits — it spins at CPU speed ([`MainPhase::Spin`]);
//! - after the loop, `fonapois()` is followed by a closing routine the tree does not have
//!   (`0x13ce`): it snaps the window to `p=0` and pan 0, compacts the last window into a
//!   200-row page shown with CRTC 9 = 1, copies that to a linear page at `0xc000` with
//!   every pixel moved to entries 192-255 and CRTC 0x13 = 0x28, and `exit(0)`s — the
//!   320×200 screen U2A's background then continues ([`HandoverStep`]);
//! - `prt` draws 29 rows (`0x1d49`) and `init` converts the strip's first 29 (`0x1857`).
//!
//! The release's `f` when the spin ends is its CPU's; the port takes the frames' answer
//! ([`SPIN_LEAVES_F_ABOVE_THRESHOLD`], A20).
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
//! - The refresh reads the source dword at `(a/4+86)*4`; under the tree's loop
//!   the last refresh is `a=316`'s, whose dword is columns 660-663 of a
//!   640-column row. The last byte, 663, is past the picture's row and comes
//!   from the *next* one; the port reads the flat picture the same way. The
//!   release's loop reaches `a=320`'s too (columns 664-667).
//! - `memset(tbuf,0,186*320)` clears the first 59,520 bytes of a 65,472-byte
//!   array, so the last 16 rows of `tbuf` keep the previous batch.
//! - The final `dofade(palette2,palette)` (`MAIN.C:147-149`) only touches
//!   entries 64-255, which the picture never indexes; it is kept as written,
//!   and runs when the loop ends on a blocking pass with `f > 7`.
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
//! `sr_assets::font::Font` from the decoded 30-row strip — the routine reads
//! 32 rows, and the two rows past the file are zero in the release's image
//! (A9). A test shows the one-byte shift
//! moves no glyph the cards use, and another that the release's font is this
//! strip but for one glyph the part never draws.

use std::cell::RefCell;
use std::rc::Rc;

use sr_assets::font::Font;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::View;
use sr_engine::Palette;

use crate::parts::{load_image, mode_view};

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
/// The strip's rows. `faddtext`/`addtext` read 32 (`MAIN.C:335`, `:428`; the release's
/// `cmp word [bp-4],20h`, image `0x24c9`), and rows 30 and 31 are zero in the release's
/// image (A9), so the 30 the file holds are everything those loops can ink.
const FONT_ROWS: usize = 30;
/// The release converts the 2-bit strip to 0x40/0x80/0xC0 in its first **29** rows
/// (`cmp word [bp-8],1dh`, image `0x1857`; the tree's `MAIN.C:169` says 32), so row 29
/// keeps its raw 1/2/3 (A9).
const FONT_CONVERTED_ROWS: usize = 29;
/// The release's `prt` draws **29** rows: `y2w=y+1dh` (`add ax,1dh`, image `0x1d49`), where
/// the tree's `MAIN.C:274` has `y+32`.
const PRT_ROWS: usize = 29;
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
/// The release's loop runs **while `a <= 320`** (`cmp word [9eh],140h; jg`, image
/// `0x1380`), where the tree's `MAIN.C:87` has `a<320`; the inner wait's own bound is still
/// `a<319` (`:138`, image `0x1341`).
const SCROLL_LAST: u16 = 320;
/// The release's `do_scroll` begins `if(a>320) return 0;` (`cmp word [9eh],140h; jle`,
/// image `0x2315`), a guard the tree's `MAIN.C:397` does not have.
const SCROLL_GUARD: u16 = 320;
/// The release's every pass ends `do_scroll(tptr<5 ? 1 : 0)` (`cmp word [0a2h],5; jge`,
/// image `0x136c`), where the tree's `MAIN.C:145` is `do_scroll(1)`: once the fifth batch
/// (the empty `case 4`) is taken, the text is no longer carried with the window, and the
/// pass no longer waits for the step.
const SCROLL_TEXT_BATCHES: usize = 5;
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
/// The release's `if(aa<304) fmaketext(aa+16)` (`cmp word [9eh],130h; jge`, image
/// `0x134f`), where the tree's `MAIN.C:141` has `320-12`, 308.
const MAKETEXT_LIMIT: u16 = 304;
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

/// What the release's `f` is when its loop ends on the spin (see [`MainPhase::Spin`]): its
/// `if(f>7)` (image `0x13a2`) reads a counter the spin has stepped once a pass at CPU speed,
/// wrapping through all 65,536 values, so the release's answer is its session's CPU time.
/// The port takes the answer the release's frames give — **not above 7, no final fade** —
/// because with the 64-tick fade ALKU would end after the original's U2A has started (boot
/// 5961, `reference/parts/fullrun`); A20 registers it.
const SPIN_LEAVES_F_ABOVE_THRESHOLD: bool = false;

// ------------------------------------------------------ the release's hand-over --

/// The release's `main` ends in a routine the tree's `MAIN.C` does not have (image
/// `0x13ce`-`0x1723`, called at `0x13bd` after `fonapois`): it rebuilds the screen as a
/// linear 320×200 page at `0xc000` and leaves the part with `exit(0)`. Its compaction reads
/// memory row `2*min(y,180)` (`cmp word [bp-8],0b4h`, image `0x14e1`; `imul 160h`).
const HANDOVER_CLAMP_ROW: usize = 180;
/// `add dx,738h` (image `0x1511`): the compacted rows start 10 rows and 88 bytes in.
const HANDOVER_COMPACT_BASE: usize = 0x738;
/// `cmp word [bp-8],0c8h` (`0x1521`, `0x159d`) rows of `push 50h` (`0x14d2`) bytes: 320
/// pixels by 200.
const HANDOVER_ROWS: usize = 200;
const HANDOVER_ROW_BYTES: usize = 0x50;
/// `mov es:[10h],0c000h` (image `0x169b`): `cop_start` on the linear page, whose rows are
/// `0x50` bytes apart (`imul 50h; add ax,0c000h`, `0x1589`).
const HANDOVER_PAGE: usize = 0xc000;
/// The recolouring loops' far pointer is `9fffh:0c000h` (`0x15be`, `0x15c3`) — video byte
/// `0xbff0` — for `0x4000` bytes a plane (`0x15dd`).
const HANDOVER_RECOLOUR_FIRST: usize = 0xbff0;
const HANDOVER_RECOLOUR_BYTES: usize = 0x4000;
/// `and al,3fh; add al,0c0h` (`0x15d0`): the picture moved to entries 192-255, which the
/// palette alias (`MAIN.C:212`) makes the same colours.
const HANDOVER_RECOLOUR_BASE: u8 = 0xC0;
/// CRTC 9 = 1 (`mov ax,0109h`, image `0x1559`) shows each memory row twice — the 320×200
/// the emulator captures — and CRTC 0x13 = 0x28 (`mov ax,2813h`, `0x16be`) is the linear
/// page's 80 bytes a line.
const HANDOVER_VIEW_HEIGHT: u16 = 200;

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

    /// `MAIN.C:173-181`: a strip value 1/2/3 becomes 0x40/0x80/0xC0 in the rows the
    /// release converts ([`FONT_CONVERTED_ROWS`]); row 29 is the raw value.
    fn ink(&self, ch: u8, column: usize, row: usize) -> u8 {
        let Some(glyph) = self.glyph(ch) else {
            return 0;
        };
        let width = self.width(ch);
        if column >= width || row >= FONT_ROWS {
            return 0;
        }
        let raw = glyph[row * width + column];
        if row >= FONT_CONVERTED_ROWS {
            // The release's row 29 is left as the file has it (image `0x1857`).
            return raw;
        }
        match raw & 3 {
            1 => 0x40,
            2 => 0x80,
            3 => 0xC0,
            _ => 0,
        }
    }
}

/// `MAIN.C:259`'s mapping from an order position to a cell: the first cell's
/// character ([`sr_assets::font::FIRST`]; `MAIN.C:40`'s order opens with `A`) plus the
/// index — the form `Font::glyph` takes.
fn cell_char(index: usize) -> char {
    char::from_u32(sr_assets::font::FIRST as u32 + index as u32).expect("a cell index is a character")
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
    /// The release's closing routine (image `0x13ce`), a step per copper frame.
    Handover(HandoverStep),
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
    /// The release's `do_scroll(0)` once `tptr` has reached 5 (image `0x136c`): the call
    /// does not wait, so the pass spins at CPU speed, stepping the window on the tick its
    /// nine frames are up and running the `f` chain in between (see
    /// [`SPIN_LEAVES_F_ABOVE_THRESHOLD`]).
    Spin,
}

/// The release's closing routine (image `0x13ce`-`0x1723`). Each step after the first
/// begins with the wait for `frame_count` the step before it set to zero
/// (`while(frame_count==0);`, `0x1406`, `0x14b0`, `0x1549`, `0x16ae`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HandoverStep {
    /// `0x1463`-`0x14a9`: `p=0`, `cop_start=a/4`, `cop_scrl=0`, `cop_pal=palette`.
    Snap,
    /// `0x14bd`-`0x1542`: write mode 1, the compaction, `cop_start=a/4+738h`.
    Compact,
    /// `0x1556`-`0x16a7`: CRTC 9 = 1, the linear copy to `0xc000`, the recolouring,
    /// `cop_start=0c000h`.
    Linear,
    /// `0x16bb`-`0x171b`: CRTC 0x13 = 0x28, the three page copies, CRTC 9 = 0x41,
    /// `close_copper`, `exit(0)`.
    Exit,
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
            shipped_palette: true,
        }
    }

    /// [`new`](Alku::new) with `HOI.U`'s own entries 1-3, for the counterfactual.
    #[cfg(test)]
    fn curated() -> Alku {
        Alku { shipped_palette: false, ..Alku::new() }
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

    /// `MAIN.C:272-291` `prt`: OR a glyph into the video memory at (`x`, `y`), the
    /// release's 29 rows ([`PRT_ROWS`]).
    fn prt(&self, video: &mut [u8], x: usize, y: usize, text: &[u8]) {
        let Some(strip) = self.strip.as_ref() else {
            return;
        };
        let mut pen = x;
        for &ch in text {
            let width = strip.width(ch);
            for column in 0..width {
                for row in 0..PRT_ROWS {
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

    /// `MAIN.C:397-412`'s non-blocking half: one step if nine frames are up. The
    /// release's `if(a>320) return 0;` comes first (image `0x2315`).
    fn do_scroll0(&mut self, video: &mut [u8]) {
        if self.a > SCROLL_GUARD {
            return;
        }
        let count = self.shared.borrow().frame_count;
        if count >= SCRLF {
            self.shared.borrow_mut().frame_count = count - SCRLF;
            self.scroll_step(video, 0);
        }
    }

    /// The blocking half: `false` when the tick was spent waiting, `true` when
    /// the call returned — with a step, or at once past the release's guard (image
    /// `0x2315`).
    fn blocking_scroll(&mut self, video: &mut [u8], mode: u8) -> bool {
        if self.a > SCROLL_GUARD {
            return true;
        }
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
                    // `MAIN.C:150`: the final `fonapois()` after the fade, then the
                    // release's closing routine (image `0x13bd`).
                    self.fonapois(video);
                    self.stage = Stage::Handover(HandoverStep::Snap);
                    return PartStatus::Continuing;
                }
                Stage::Handover(_) => return PartStatus::Continuing,
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
                    if self.tptr >= SCROLL_TEXT_BATCHES {
                        // The release's `do_scroll(0)` (image `0x1377`): the pass
                        // stops waiting for the step.
                        self.begin_spin(video);
                        if self.a > SCROLL_LAST {
                            return self.end_loop(video, SPIN_LEAVES_F_ABOVE_THRESHOLD);
                        }
                        return PartStatus::Continuing;
                    }
                    if !self.blocking_scroll(video, 1) {
                        return PartStatus::Continuing;
                    }
                    if self.a > SCROLL_LAST {
                        return self.end_loop(video, self.f > FINAL_FADE_THRESHOLD);
                    }
                    self.stage = Stage::Main(MainPhase::Branch);
                }
                Stage::Main(MainPhase::Spin) => {
                    self.do_scroll0(video);
                    if self.a > SCROLL_LAST {
                        return self.end_loop(video, SPIN_LEAVES_F_ABOVE_THRESHOLD);
                    }
                    return PartStatus::Continuing;
                }
                _ => return PartStatus::Finished,
            }
        }
    }

    /// The first pass of the release's spin (see [`MainPhase::Spin`]), on the tick the
    /// fifth batch ends. Its `do_scroll(0)` steps only if nine frames are up; the passes
    /// after it take the chain's first two arms within microseconds — `f==0` sets the
    /// `textin` fade and adds 20, thirty passes take `f` to 50, which sets the `textout`
    /// fade (`MAIN.C:89-98`, image `0x1134`-`0x1181`) — and from there `f` counts passes at
    /// CPU speed, which the port does not: it holds `f` at 51 and takes the loop's end from
    /// [`SPIN_LEAVES_F_ABOVE_THRESHOLD`]. The fades touch entries 64-255 only, which no
    /// pixel uses once the fifth batch's `ffonapois` has cleared the text.
    fn begin_spin(&mut self, video: &mut [u8]) {
        self.do_scroll0(video);
        if self.a > SCROLL_LAST {
            return;
        }
        if self.f == 0 {
            self.set_fade_source(FadeSource::TextIn);
            self.shared.borrow_mut().cop_dofade = TEXT_FADE_STEPS;
            self.f = 50;
        }
        if self.f == 50 {
            self.set_fade_source(FadeSource::TextOut);
            self.shared.borrow_mut().cop_dofade = TEXT_FADE_STEPS;
            self.f = 51;
        }
        self.stage = Stage::Main(MainPhase::Spin);
    }

    /// The loop has ended (`a > 320`, image `0x1380`). The release's `if(dis_exit())`
    /// call of its closing routine (`0x1394`) is the port's exit path in [`Part::frame`];
    /// then `if(f>7) dofade(palette2,palette)` (`0x13a2`, `MAIN.C:147-149`), `fonapois()`
    /// (`0x13b8`) and the closing routine (`0x13bd`).
    fn end_loop(&mut self, video: &mut [u8], fade: bool) -> PartStatus {
        if fade {
            // The final fade's first step is this tick's (the rest are the
            // `Stage::FinalFade` arm).
            self.dofade_step(&self.palette2, &self.palette, 0);
            self.shared.borrow_mut().frame_count = 0;
            self.stage = Stage::FinalFade { a: 1 };
            return PartStatus::Continuing;
        }
        self.fonapois(video);
        self.stage = Stage::Handover(HandoverStep::Snap);
        PartStatus::Continuing
    }

    /// One tick of the release's closing routine (image `0x13ce`-`0x1723`).
    ///
    /// Each step's CRTC writes are the part's own `out`s, made right after the wait for the
    /// copper's frame — in the retrace, so on the hardware they reach the frame the start
    /// address `copper1` wrote before it does. The start is slot 1's, which the next tick's
    /// frame reads. The engine sizes a frame from the view as the tick begins, so CRTC 9's
    /// 200 lines are written on the tick that sets the compacted start (its own frame still
    /// composes 400 lines at the old start); CRTC 0x13's stride, which the compose reads at
    /// once, is written on the tick whose frame first reads the linear page. (The emulator
    /// defers a CRTC 9 change by half a frame, `dosbox-automation`'s `VGA_StartResize`, so
    /// its capture holds one frame of the compacted page that turns double-scanned at line
    /// 185 — the original's 5950, `reference-indices.md` §4.5, which
    /// `the_originals_5950_is_the_compacted_page_resized_mid_frame` matches line for line;
    /// A20.)
    ///
    /// The three page copies of the last step (`0x16c2`-`0x1703`, 16,000 bytes from
    /// `9fffh:0c000h` to `0a000h:0`, `:4000h` and `9fffh:8000h`) are not made: nothing
    /// displays them before the part exits (the view stays on `0xc000`), and the port's
    /// video memory is 256,000 pixels (D16), so the page at `0xc000` wraps its last 4,608
    /// pixels onto pixel 0, which the first copy would overwrite on screen.
    fn handover_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let Stage::Handover(step) = self.stage else {
            return PartStatus::Continuing;
        };
        if step != HandoverStep::Snap && self.shared.borrow().frame_count == 0 {
            return PartStatus::Continuing;
        }
        let a = usize::from(self.a);
        let video = ctx.surface.pixels_mut();
        match step {
            HandoverStep::Snap => {
                // `0x142a`-`0x1460`: three `memcpy(palette+64*3*k, palette, 192)`, which
                // repeat what `MAIN.C:212`'s alias already holds.
                self.p = 0;
                let palette = self.palette;
                let mut shared = self.shared.borrow_mut();
                shared.cop_start = (a / 4) as u16;
                shared.cop_scrl = 0;
                for (entry, rgb) in palette.iter().enumerate() {
                    shared.cop_pal[entry * 3..entry * 3 + 3].copy_from_slice(rgb);
                }
                shared.do_pal = true;
                shared.frame_count = 0;
                drop(shared);
                self.stage = Stage::Handover(HandoverStep::Compact);
            }
            HandoverStep::Compact => {
                // Write mode 1: a latched copy of all four planes, 80 bytes a row.
                for y in 0..HANDOVER_ROWS {
                    let to = a / 4 + y * LINE_BYTES + HANDOVER_COMPACT_BASE;
                    let from = a / 4 + y.min(HANDOVER_CLAMP_ROW) * 2 * LINE_BYTES;
                    copy_bytes(video, from, to, HANDOVER_ROW_BYTES);
                }
                let mut shared = self.shared.borrow_mut();
                shared.cop_start = (a / 4 + HANDOVER_COMPACT_BASE) as u16;
                shared.frame_count = 0;
                drop(shared);
                // `0x1556`: CRTC 9 = 1, each memory row shown twice (the next frame's).
                ctx.view.height = HANDOVER_VIEW_HEIGHT;
                self.stage = Stage::Handover(HandoverStep::Linear);
            }
            HandoverStep::Linear => {
                for y in 0..HANDOVER_ROWS {
                    let to = HANDOVER_PAGE + y * HANDOVER_ROW_BYTES;
                    let from = a / 4 + y * LINE_BYTES + HANDOVER_COMPACT_BASE;
                    copy_bytes(video, from, to, HANDOVER_ROW_BYTES);
                }
                // Write mode 0, a plane at a time (the order cannot matter).
                let len = video.len();
                for byte in
                    HANDOVER_RECOLOUR_FIRST..HANDOVER_RECOLOUR_FIRST + HANDOVER_RECOLOUR_BYTES
                {
                    for plane in 0..PIXELS_PER_BYTE {
                        let at = (byte * PIXELS_PER_BYTE + plane) % len;
                        video[at] = (video[at] & FADE_MASK).wrapping_add(HANDOVER_RECOLOUR_BASE);
                    }
                }
                let mut shared = self.shared.borrow_mut();
                shared.cop_start = HANDOVER_PAGE as u16;
                shared.frame_count = 0;
                drop(shared);
                self.stage = Stage::Handover(HandoverStep::Exit);
            }
            HandoverStep::Exit => {
                // `0x16bb`: CRTC 0x13 = 0x28, the linear page's 80 bytes a line.
                ctx.view.y_line_offset = Some((HANDOVER_ROW_BYTES * PIXELS_PER_BYTE) as u16);
                // `0x170d` CRTC 9 = 0x41 keeps the double scan; `close_copper` (`0x1714`)
                // and `exit(0)` (`0x171b`) are the part's end.
                self.stage = Stage::Done;
                return PartStatus::Finished;
            }
        }
        PartStatus::Continuing
    }

    /// Read the assets and prepare the screen; see the module docs.
    fn prepare(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part waits on the music's start codes (ALKU/MAIN.C:59, :66, :72, \
             :79, :138) and the run has no music to wait on",
        )?;
        let picture = load_image(NAME, HZPIC_PATH)?;
        if (picture.width() as usize, picture.height() as usize)
            != (HZPIC_WIDTH, HZPIC_HEIGHT)
        {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{HZPIC_PATH} is {}x{}; the part reads {HZPIC_WIDTH}x{HZPIC_HEIGHT}",
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

        let strip_image = load_image(NAME, FONT_PATH)?;
        if (strip_image.width(), strip_image.height()) != (1500, FONT_ROWS as u16) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{FONT_PATH} is {}x{}; the strip is 1500x{FONT_ROWS}",
                    strip_image.width(),
                    strip_image.height()
                ),
            });
        }
        let font = Font::from_image(strip_image);
        self.strip = Some(Strip::scan(font).map_err(|message| PartError {
            part: NAME,
            message: format!("{FONT_PATH}: {message}"),
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
        // The mode set's display state (`TWEAK.ASM:18-54`): 320×400, a
        // 704-pixel line.
        *ctx.view = mode_view(VIEW_WIDTH, VIEW_HEIGHT, Some(LINE_PIXELS as u16), Some(0));
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

/// A latched copy (write mode 1): `count` plane bytes from `from` to `to`, all four planes,
/// in ascending order as the release's byte loop (image `0x1724`) makes it. Addresses past
/// the port's video memory wrap, as the view reads them.
fn copy_bytes(video: &mut [u8], from: usize, to: usize, count: usize) {
    let len = video.len();
    for byte in 0..count {
        for plane in 0..PIXELS_PER_BYTE {
            let value = video[((from + byte) * PIXELS_PER_BYTE + plane) % len];
            video[((to + byte) * PIXELS_PER_BYTE + plane) % len] = value;
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
        if matches!(self.stage, Stage::Handover(_)) {
            return self.handover_tick(ctx);
        }
        let code = ctx.music().map_or(i32::MIN, |music| music.sync());
        let tick = self.tick;
        let status = self.tick_stage(ctx.surface.pixels_mut(), code, tick);
        // The loop's end runs straight into the closing routine's first step.
        if self.stage == Stage::Handover(HandoverStep::Snap) {
            return self.handover_tick(ctx);
        }
        status
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{differing_pixels, reference, Rig, ScriptedMusic};
    use sr_engine::{dac_expand, Surface};


    /// A rig whose player answers `sync()` from `codes`, one a tick.
    fn rig(codes: Vec<i32>) -> Rig {
        Rig::new(320, 200).with(ScriptedMusic::sync_codes(codes))
    }

    fn started_with(codes: Vec<i32>) -> (Rig, Alku) {
        let mut rig = rig(codes);
        let mut part = Alku::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Alku) {
        started_with(Vec::new())
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

    // --- the gates and the scroll ------------------------------------------

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
        let (mut rig, mut part) = started_with(script);
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
        let (mut rig, mut part) = started_with(picture_script(1500));
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
        let image = load_image(NAME, FONT_PATH).unwrap();
        assert_eq!((image.width(), image.height()), (1500, 30));
        let font = Font::from_image(image);
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
        let image = load_image(NAME, HZPIC_PATH).unwrap();
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
        let (mut rig, mut part) = started_with(picture_script(1500));
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
        let mut rig = rig(Vec::new());
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
        let (mut rig, mut part) = started_with(picture_script(1500));
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
        let (mut rig, mut part) = started_with(picture_script(1500));
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
        let (mut rig, mut part) = started_with(vec![0; 10]);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.stage, Stage::Done);
    }

    // --- the reference frames ----------------------------------------------

    /// The reference frame at boot `boot` is the tick `boot - 404`'s composed
    /// frame, which is what [`Rig::step_render`] captures.
    fn render_at(boot: u32) -> Vec<u8> {
        let tick = boot - 404;
        let (mut rig, mut part) = started_with(trace_script(tick));
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
                differing_pixels(&frame, &reference("alku", boot, "rgb")),
                0,
                "boot {boot}: the held frame is not the reference's"
            );
        }
    }

    #[test]
    fn the_scrolling_keyframe_is_the_references() {
        // The release's loop (A20): the fifth batch's `ffonapois` clears the text and
        // its `do_scroll(tptr<5 ? 1 : 0)` no longer carries it, so 5859 is bare.
        let key = render_at(5859);
        assert_eq!(differing_pixels(&key, &reference("alku", 5859, "rgb")), 0, "boot 5859");
        // The window steps every nine ticks and nothing else moves: the port's 5858 is
        // the same picture and its 5860 the next step's (14,048 pixels), where the
        // reference's own session holds 5859-5861 (`reference-indices.md` §4.4).
        assert_eq!(render_at(5858), key, "5858 is the same step");
        assert_eq!(differing_pixels(&render_at(5860), &key), 14048, "5860 is the next step");
    }

    /// The trace's run, stepped until `stop` holds or `last` ticks have run.
    fn run_until(last: u32, stop: impl Fn(&Alku) -> bool) -> (Rig, Alku) {
        let (mut rig, mut part) = started_with(trace_script(last));
        while !stop(&part) && rig.tick <= last {
            rig.step(&mut part);
        }
        (rig, part)
    }

    #[test]
    fn the_fifth_batch_stops_carrying_the_text_and_the_pass_no_longer_waits() {
        // The trace's fifth batch (`case 4`) ends at a=308 on tick 5429 (boot 5833),
        // past the release's 304, so no `fmaketext`; the spin's first pass sets the
        // `textin` and then the `textout` fade.
        let (mut rig, mut part) =
            run_until(6000, |part| part.stage == Stage::Main(MainPhase::Spin));
        assert_eq!(part.stage, Stage::Main(MainPhase::Spin));
        assert_eq!((rig.tick - 1, part.a, part.tptr, part.f), (5429, 308, 5, 51));
        assert_eq!(part.shared.borrow().cop_dofade, TEXT_FADE_STEPS);
        let dtau = part.dtau.clone();
        // No video byte in the text's rows keeps a text bit, and none gains one: the
        // window steps every nine ticks without `ascrolltext`.
        let text_bits = |rig: &Rig| {
            let first = (TEXT_BASE_ROW + TEXT_FIRST_ROW) * LINE_PIXELS;
            let end = (TEXT_BASE_ROW + TEXT_FIRST_ROW + TEXT_ROWS) * LINE_PIXELS;
            rig.surface.pixels()[first..end].iter().filter(|&&v| v & !FADE_MASK != 0).count()
        };
        assert_eq!(text_bits(&rig), 0, "the fifth batch's ffonapois cleared the text");
        let a = part.a;
        for _ in 0..90 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
            assert_eq!(text_bits(&rig), 0);
        }
        assert_eq!(part.a, a + 10, "ten steps in ninety ticks: the cadence is unchanged");
        assert_eq!(part.dtau, dtau, "no fmaketext ran");
    }

    #[test]
    fn the_loop_ends_past_320_without_the_final_fade_and_hands_over_in_four_ticks() {
        let (mut rig, mut part) = run_until(6000, |part| matches!(part.stage, Stage::Handover(_)));
        // The release's `a<=320` (image `0x1380`): the step from 320 is the last.
        assert_eq!(part.a, 321);
        assert_eq!(rig.tick - 1, 5545, "boot 5949");
        assert_eq!(part.stage, Stage::Handover(HandoverStep::Compact), "the snap ran");
        assert_eq!(part.p, 0);
        assert_eq!(part.shared.borrow().cop_start, 80);
        assert_eq!(part.shared.borrow().cop_scrl, 0);
        let mut steps = Vec::new();
        loop {
            let status = rig.step(&mut part);
            steps.push((404 + rig.tick - 1, part.stage, status));
            if status == PartStatus::Finished {
                break;
            }
        }
        assert_eq!(
            steps,
            [
                (5950, Stage::Handover(HandoverStep::Linear), PartStatus::Continuing),
                (5951, Stage::Handover(HandoverStep::Exit), PartStatus::Continuing),
                (5952, Stage::Done, PartStatus::Finished),
            ],
            "one step a copper frame, and no 64-tick fade"
        );
    }

    /// The frame the tick composes — at the view's geometry as the tick began, as
    /// `Engine::tick` sizes it — and the view's start and pan.
    fn step_render_view(rig: &mut Rig, part: &mut Alku) -> (u16, u16, u32, u8, Vec<u8>) {
        let (width, height) = (rig.view.width, rig.view.height);
        rig.run_slot(0);
        let tick = rig.tick;
        part.frame(&mut rig.ctx(tick));
        let (start, pan) = (rig.view.start_addr, rig.view.pan);
        let mut out = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let at = rig.view.source_index(&rig.surface, x, y);
                let entry = rig.palette.entry(rig.surface.pixels()[at]);
                out.extend(entry.iter().map(|&c| dac_expand(c)));
            }
        }
        rig.run_slot(1);
        rig.run_slot(2);
        rig.tick += 1;
        (width, height, start, pan, out)
    }

    #[test]
    fn the_closing_routine_shows_the_last_window_as_a_320x200_page() {
        let (mut rig, mut part) = run_until(5544, |_| false);
        let frames: Vec<_> = (0..4).map(|_| step_render_view(&mut rig, &mut part)).collect();
        // 5949 is a=319's window (byte 79 + 88, pan 3); 5950 is a=320's, which the snap
        // repeats (`p=0`, `cop_start=a/4` = 80, pan 0) — both at 320×400.
        assert_eq!((frames[0].0, frames[0].1, frames[0].2, frames[0].3), (320, 400, 4 * 167, 3));
        assert_eq!((frames[1].0, frames[1].1, frames[1].2, frames[1].3), (320, 400, 4 * 80, 0));
        // 5951: CRTC 9 = 1 over the compacted rows — each 400-line pair is one row.
        assert_eq!((frames[2].0, frames[2].1), (320, HANDOVER_VIEW_HEIGHT));
        assert_eq!(frames[2].2, (4 * (80 + HANDOVER_COMPACT_BASE)) as u32);
        let tall = &frames[1].4;
        let row = 320 * 3;
        for y in 0..200usize {
            assert_eq!(
                &frames[2].4[y * row..(y + 1) * row],
                &tall[2 * y * row..(2 * y + 1) * row],
                "row {y} is the 400-line frame's row {}",
                2 * y
            );
        }
        // 5952: the linear page at 0xc000, 80 bytes a line, every pixel moved to
        // entries 192-255 — the same colours under the palette alias.
        assert_eq!(rig.view.start_addr, (HANDOVER_PAGE * PIXELS_PER_BYTE) as u32);
        assert_eq!(rig.view.y_line_offset, Some(320));
        assert_eq!(frames[3].4, frames[2].4, "the linear page is the same picture");
        assert_eq!((frames[3].0, frames[3].1), (320, HANDOVER_VIEW_HEIGHT));
        for y in 0..200u16 {
            for x in 0..320u16 {
                let at = rig.view.source_index(&rig.surface, x, y);
                assert!(rig.surface.pixels()[at] >= HANDOVER_RECOLOUR_BASE, "({x},{y})");
            }
        }
    }

    #[test]
    fn the_final_fade_runs_sixty_four_ticks_when_the_blocking_loop_leaves_f_above_seven() {
        // With no code past 4 the first batch waits out the window (`a<319`), so the
        // loop ends on the blocking pass with `tptr` 1 and `f` counted: 20 > 7.
        let last = 6000;
        let (mut rig, mut part) = started_with(picture_script(last));
        while !matches!(part.stage, Stage::FinalFade { .. }) && rig.tick <= last {
            rig.step(&mut part);
        }
        assert!(
            matches!(part.stage, Stage::FinalFade { .. }),
            "the loop leaves f above 7, so the final fade runs; got {:?} at f={}",
            part.stage,
            part.f
        );
        assert_eq!((part.a, part.tptr, part.f), (321, 1, 20));
        let mut fade_ticks = 0u32;
        while matches!(part.stage, Stage::FinalFade { .. }) {
            fade_ticks += 1;
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
            assert!(fade_ticks <= 64, "the fade ran past 64 ticks");
        }
        assert_eq!(fade_ticks, 64, "the final fade is the source's 64 steps");
        assert_eq!(part.stage, Stage::Handover(HandoverStep::Compact));
        let mut status = PartStatus::Continuing;
        for _ in 0..3 {
            status = rig.step(&mut part);
        }
        assert_eq!(status, PartStatus::Finished, "and then the closing routine");
    }

    /// The release's image holds what the port names as constants (D49): the loop, its
    /// closing routine and the text routines, compared byte for byte at the offsets the
    /// constants cite (`out/glitches/alku-report.md`; `docs/fidelity/alku.md` §12).
    #[test]
    fn the_release_loop_and_its_closing_routine_are_the_images() {
        let file = std::fs::read("../../assets/MAIN/DATA/ALKU.EXE").unwrap();
        let image = sr_assets::exe::unpack(&file).unwrap();
        let at = |offset: usize, bytes: &[u8]| {
            assert_eq!(&image.image[offset..offset + bytes.len()], bytes, "image {offset:#x}");
        };
        // `for(f=60;a<=320 && !dis_exit();)`: `cmp word [9eh],140h; jg`.
        at(0x1380, &[0x81, 0x3e, 0x9e, 0x00, 0x40, 0x01, 0x7f]);
        assert_eq!(u16::from_le_bytes([image.image[0x1384], image.image[0x1385]]), SCROLL_LAST);
        // The batch wait's `a<319`, the tree's: `cmp word [9eh],13fh; jl`.
        at(0x1341, &[0x81, 0x3e, 0x9e, 0x00, 0x3f, 0x01, 0x7c]);
        // `if(a<304) fmaketext(aa+16)`: `cmp word [9eh],130h; jge`, `add ax,10h`.
        at(0x134f, &[0x81, 0x3e, 0x9e, 0x00, 0x30, 0x01, 0x7d, 0x0b, 0x05, 0x10, 0x00]);
        assert_eq!(u16::from_le_bytes([image.image[0x1353], image.image[0x1354]]), MAKETEXT_LIMIT);
        // `do_scroll(tptr<5 ? 1 : 0)`: `cmp word [0a2h],5; jge; push 1; jmp; push 0`.
        at(0x136c, &[0x83, 0x3e, 0xa2, 0x00, 0x05, 0x7d, 0x04, 0x6a, 0x01, 0xeb, 0x02, 0x6a, 0x00]);
        assert_eq!(usize::from(image.image[0x1370]), SCROLL_TEXT_BATCHES);
        // After the loop: `if(f>7)` dofade, `fonapois()`, the closing routine (cs:04ceh).
        at(0x13a2, &[0x83, 0x7e, 0xfc, 0x07, 0x7e]);
        assert_eq!(i32::from(image.image[0x13a5]), FINAL_FADE_THRESHOLD);
        at(0x13b8, &[0x9a, 0x1e, 0x0d, 0xf0, 0x00, 0x9a, 0xce, 0x04, 0xf0, 0x00, 0xc9, 0xcb]);
        // `do_scroll`'s guard: `cmp word [9eh],140h; jle`.
        at(0x2315, &[0x81, 0x3e, 0x9e, 0x00, 0x40, 0x01, 0x7e]);
        assert_eq!(u16::from_le_bytes([image.image[0x2319], image.image[0x231a]]), SCROLL_GUARD);
        // The closing routine: the clamp, the compacted base, CRTC 9, the page, the
        // recolouring, CRTC 0x13.
        at(0x14e1, &[0x81, 0x7e, 0xf8, 0xb4, 0x00]);
        assert_eq!(usize::from(image.image[0x14e4]), HANDOVER_CLAMP_ROW);
        at(0x1511, &[0x81, 0xc2, 0x38, 0x07]);
        assert_eq!(
            usize::from(u16::from_le_bytes([image.image[0x1513], image.image[0x1514]])),
            HANDOVER_COMPACT_BASE
        );
        at(0x1521, &[0x81, 0x7e, 0xf8, 0xc8, 0x00]);
        assert_eq!(usize::from(image.image[0x1524]), HANDOVER_ROWS);
        at(0x14d2, &[0x6a, 0x50]);
        assert_eq!(usize::from(image.image[0x14d3]), HANDOVER_ROW_BYTES);
        at(0x1556, &[0xba, 0xd4, 0x03, 0xb8, 0x09, 0x01, 0xef]);
        at(0x15be, &[0xc7, 0x46, 0xfe, 0xff, 0x9f, 0xc7, 0x46, 0xfc, 0x00, 0xc0]);
        assert_eq!(0x9fff0 + 0xc000 - 0xa0000, HANDOVER_RECOLOUR_FIRST);
        at(0x15dd, &[0x81, 0x7e, 0xfa, 0x00, 0x40]);
        assert_eq!(
            usize::from(u16::from_le_bytes([image.image[0x15e0], image.image[0x15e1]])),
            HANDOVER_RECOLOUR_BYTES
        );
        at(0x15d0, &[0x24, 0x3f, 0x04, 0xc0]);
        assert_eq!(image.image[0x15d3], HANDOVER_RECOLOUR_BASE);
        at(0x169b, &[0x26, 0xc7, 0x06, 0x10, 0x00, 0x00, 0xc0]);
        assert_eq!(
            usize::from(u16::from_le_bytes([image.image[0x16a0], image.image[0x16a1]])),
            HANDOVER_PAGE
        );
        at(0x16bb, &[0xba, 0xd4, 0x03, 0xb8, 0x13, 0x28, 0xef]);
        assert_eq!(usize::from(image.image[0x16c0]) * 2, HANDOVER_ROW_BYTES);
        // The text: `prt`'s `y2w=y+1dh`, the conversion's and the scan's 29 rows,
        // `faddtext`'s 32.
        at(0x1d46, &[0x8b, 0x46, 0x08, 0x05, 0x1d, 0x00]);
        assert_eq!(usize::from(image.image[0x1d4a]), PRT_ROWS);
        at(0x1857, &[0x83, 0x7e, 0xf8, 0x1d]);
        assert_eq!(usize::from(image.image[0x185a]), FONT_CONVERTED_ROWS);
        at(0x1acf, &[0x83, 0x7e, 0xf8, 0x1d]);
        at(0x24c9, &[0x83, 0x7e, 0xfc, 0x20]);
    }

    /// A9, read from the image: the release's font (segment `4254h`, read at `es:[bx+1]`)
    /// is `FONA.UH[783:]` in rows 0-29 but for three bytes, and rows 30 and 31 are zero.
    /// The three bytes join the tree's two 3-column glyphs at 938 and 944 into one
    /// 9-column glyph, so the release's `fonaorder` has 75 characters where the tree's has
    /// 76; every glyph the part draws is the same bytes at the same run.
    #[test]
    fn the_release_font_is_the_strip_but_for_one_glyph_and_its_padding_is_zero() {
        let file = std::fs::read("../../assets/MAIN/DATA/ALKU.EXE").unwrap();
        let image = sr_assets::exe::unpack(&file).unwrap().image;
        let strip = std::fs::read("../../upstream/SecondReality/ALKU/FONA.UH").unwrap();
        let font = 0x42541;
        let mut differing = Vec::new();
        for row in 0..30usize {
            for x in 0..1500usize {
                let shipped = image[font + row * 1500 + x];
                let tree = strip[783 + row * 1500 + x];
                if shipped != tree {
                    differing.push((row, x, tree, shipped));
                }
            }
        }
        assert_eq!(differing, [(0, 941, 0, 1), (0, 943, 0, 1), (1, 942, 0, 1)]);
        assert!(image[font + 30 * 1500..font + 32 * 1500].iter().all(|&b| b == 0), "rows 30-31");
        // The release's `fonaorder` (DGROUP `0c4h`, the far pointer at `9ah`) and the
        // card that draws the two logos (`'['`, `']'`, DGROUP `14bh`/`14dh`).
        let dgroup = 0x50fb0;
        assert_eq!(&image[dgroup + 0x9a..dgroup + 0x9e], &[0xc4, 0x00, 0xfb, 0x50]);
        let order = &image[dgroup + 0xc4..dgroup + 0xc4 + 76];
        assert_eq!(
            order,
            b"ABCDEFGHIJKLMNOPQRSTUVWXabcdefghijklmnopqrstuvwxyz0123456789!?,.:&()+-*=/[]\0"
        );
        assert_eq!(&image[dgroup + 0x14b..dgroup + 0x14f], b"[\0]\0");
        // The tree's order names the two logos by their run, 74 and 75 — the release's
        // '[' and ']' after the merged glyph.
        assert_eq!(FONT_ORDER.len(), 76);
        assert_eq!(&FONT_ORDER[74..], b"\x8f\x99");
        assert_eq!(&order[73..75], b"[]");
    }

    /// The D24 measurement, ignored: with the curated `HOI.U` entries the
    /// reference's cards and its picture band differ; with the shipped
    /// entries they do not. Run with `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_curated_palette_entries_leave_the_reference_different() {
        let mut rig = rig(trace_script(5455));
        let mut part = Alku::curated();
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
            let expected = reference("alku", boot, "rgb");
            let differing = differing_pixels(&frame, &expected);
            println!(
                "boot {boot}: {differing} of 128000 pixels differ with the curated entries"
            );
            measured.push((boot, differing));
        }
        // A20's numbers, pinned: a silent change here would let the note's
        // counterfactual go stale.
        assert_eq!(measured, [(900, 0), (1719, 1595), (2719, 4171), (5859, 966)]);
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
        let mut rows = Vec::new();
        for shift in -60..=60 {
            let tick = 5859 - 404;
            let (mut rig, mut part) =
                started_with(trace_script_after_gate(tick, shift));
            while rig.tick < tick {
                rig.step(&mut part);
            }
            let (_, frame) = rig.step_render(&mut part);
            let expected = reference("alku", 5859, "rgb");
            let differing = differing_pixels(&frame, &expected);
            println!(
                "shift {shift:+3}: {differing:5} pixels differ; a={} p={} f={} tptr={} stage={:?}",
                part.a, part.p, part.f, part.tptr, part.stage
            );
            rows.push((shift, differing));
        }
        // A20's measurement, pinned: every phase up to 45 frames later than the trace's
        // has taken the fifth batch by 5859 and is bare; from 46 the fourth batch's
        // text is still up.
        for &(shift, differing) in &rows {
            if shift <= 45 {
                assert_eq!(differing, 0, "shift {shift:+}");
            } else {
                assert_eq!(differing, 2065, "shift {shift:+}");
            }
        }
    }

    /// The original's 5950 (`out/b3-alku-boundary2`, an unfiled capture, `reference-indices.md`
    /// §4.5) is the closing routine's compacted page: its lines 0-184 are the page's rows 0-184
    /// at 400 lines and its lines 185-399 the page's row `L/2` — the emulator applying CRTC 9 = 1
    /// half a frame late (`VGA_StartResize`), in the middle of the frame. Every line is the
    /// port's page byte for byte. Ignored: the capture is under `out/`. Run with `--ignored`.
    #[test]
    #[ignore]
    fn the_originals_5950_is_the_compacted_page_resized_mid_frame() {
        let (rig, part) = run_until(5546, |_| false);
        assert_eq!(part.stage, Stage::Handover(HandoverStep::Linear), "the page is compacted");
        let reference = std::fs::read("../../out/b3-alku-boundary2/frame_005950.rgb").unwrap();
        let start = 4 * (80 + HANDOVER_COMPACT_BASE);
        for line in 0..400usize {
            let row = if line < 185 { line } else { line / 2 };
            for x in 0..320usize {
                let index = rig.surface.pixels()[start + row * LINE_PIXELS + x];
                let entry = rig.palette.entry(index);
                let at = (line * 320 + x) * 3;
                assert_eq!(
                    [dac_expand(entry[0]), dac_expand(entry[1]), dac_expand(entry[2])][..],
                    reference[at..at + 3],
                    "line {line}, column {x}: page row {row}"
                );
            }
        }
    }
}
