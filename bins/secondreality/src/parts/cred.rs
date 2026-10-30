//! CRED — the 21 credit screens: a picture, a centred caption, and an iris.
//!
//! **The port is the shipped build's data**, read from its own image (`docs/decisions.md` D49):
//! `sr_assets::exe::unpack(assets/MAIN/DATA/CRED.EXE)` gives the load image DOS runs, a Borland
//! C++ large-model program whose DGROUP is image `0x67520` (`mov dx,6752h` at image 0). The 21
//! screens are `main`'s 21 `screenin(pic, text)` calls (`0x193a`, `0x1955`, …, `0x1b56`, 27
//! bytes apart), each read off its own bytes ([`SCREENIN_CALL`]): the picture is the `FC FC` blob
//! at the pushed segment ×16 (segments `0x1145` to `0x6339`, 0x419 paragraphs apart), and the
//! caption the NUL-separated lines at DGROUP + the pushed offset. The font raster (image `0x29f0`)
//! and `fonaorder` (DGROUP `0x96`) are read from the image too; both are the tree's byte for byte.
//! Nine of the 21 screens are not the tree's (`APPROXIMATIONS.md` A10, retired by this reading).
//!
//! The code is `CREDITS/MAIN.C` (318 lines) over `CREDITS/TWEAK.ASM` (357 lines — the `tw_*`
//! library PAM and ALKU share, whose six entry points this port keeps together and named after
//! the original), and the image's `main` (`0x190c`), `screenin` (`0x1b69`), `prt`/`prtc`
//! (`0x1d57`/`0x1e01`), `init` (`0x1e63`) and `tw_*` (segment `0x271`) are that code, with three
//! differences the port does not model (`docs/fidelity/cred.md` §9): a `printf` at `main`'s start,
//! a `while(inp(0x3da)&1);` in place of each wipe step's `for(a=0;a<10000;a++);` with
//! `tw_setstart` now before `tw_setsplit`, and an `in al,dx` from `0x3da` before
//! `tw_opengraph`'s attribute write. The loops' counts are the tree's. Its acts, in order:
//!
//! 1. `:37-39` — `dis_partstart`, `init()`, `tw_opengraph`: the tweak mode, and
//!    video memory cleared (`TWEAK.ASM:29-34`: 32,768 word writes with the map
//!    mask open is 65,536 byte addresses × 4 planes = 262,144 pixels, the whole
//!    256 KB).
//! 2. `:41-156` — 21 calls to `screenin(pic, text)`, one per screen.
//! 3. Each `screenin` (`:161-213`): `tw_setsplit(400)` (parked, below the
//!    display), `tw_clrscr`, `tw_setstart(160*200)`, one vertical blank,
//!    `tw_setpalette(&pic[0][16])`, `pic = &pic[0][784]`, the caption centred by
//!    the font strip's own metrics (`:172`), the picture laid in as
//!    `tw_putpixel(400+x, 400+y*2, pic[y][x]+16)` (`:174`), an iris **open**
//!    (`:176-192`, `for(y=200*128;y>0;y=y*12L/13)`), a **200-frame hold**
//!    (`:194`), and an iris **close** (`:196-212`,
//!    `for(y=0,v=0;y<128*200;y=y+v,v+=15)`).
//!
//! `init` is `MAIN.C:246-318`: the font strip's metrics scan, **and the palette
//! repair** — a `memmove` of each picture's palette plus a ten-entry grey ramp
//! ([`repair_palette`]) — which is part of the screen rather than setup, and is
//! ported as such.
//!
//! # The display geometry, which is not plain 320×200 mode 13h
//!
//! `tw_opengraph` (`TWEAK.ASM:18-60`) sets mode 13h, chain-4 **off**
//! (`:23-25`), CRTC 0x13 = `0x5013` ("640 wide", `:43-44`), CRTC 9 = `0x0109`
//! (scan doubling on, `:46-47`), CRTC 0x18 = `0x64` with CRTC 7's bit 4
//! (`:49-52`) and AC 0x30 = `0x61`. The pen (`tw_putpixel`, `TWEAK.ASM:87-117`)
//! writes plane `x & 3` at byte address `y*80 + x/4`, so a **logical line is
//! 80 bytes = 320 pixels**, while the display advances 160 bytes a row:
//!
//! ```text
//! 160 planar bytes per displayed row = 4 planes x 40 bytes
//!   = 2 logical lines of 80 bytes = 640 pixels at 4 pixels a byte
//! ```
//!
//! One displayed row is therefore *every other* logical line, and the port's
//! view is `origin = 320 x displayed logical line`, `y_line_offset = Some(640)`,
//! `width = 320`, `height = 200` — the layout D16's widening exists for. The
//! part writes **flat** through [`Surface::pixels_mut`] at `LINE_LEN = 320`
//! (Part B's second convention): the stride is deliberately twice the width,
//! which is exactly the shear the first convention's warning is about, done on
//! purpose and with the arithmetic above written down.
//!
//! ## Where the picture and the caption land
//!
//! The pen address of picture pixel (`x`, `y`) is `tw_putpixel(400+x, 400+2y)`,
//! i.e. flat pixel `320*(400+2y) + 400 + x` = `320*(401+2y) + 80 + x` — so the
//! picture's row 0 lands in **logical line 401** (an odd line) at column 80, and
//! its 160 columns are displayed at columns 80-239. The caption is drawn at
//! logical lines 16, 58, 100, … (even ones), which is why it is displayed from
//! the line compare down, where the address counter restarts at 0: every other
//! glyph row shows, and a caption is 16 displayed rows tall.
//!
//! # The iris: three registers, stepped once a vertical blank
//!
//! The parked state (`:165-167`) is `tw_setsplit(400)` — line compare 400, below
//! the bottom of a 200-row display, so inert — `tw_clrscr`, and
//! `tw_setstart(160*200)`: 32,000 bytes = 400 logical lines = 128,000 pixels.
//!
//! Each step of either wipe is the same three writes ([`Display`]), and the two
//! loops are their `y` sequences ([`open_ramp`], [`close_ramp`]). The open
//! loop's first step writes **the parked state** (`y = 200*128` → line compare
//! 400, `yy = 0` → start 32,000, pan 0) and its last writes the **hold's**
//! (`y = 1` → 200, `yy = 320` → 32,080 bytes, pan 0); the close loop's first
//! step is the hold's too, and its last is the state the next screen's tick 0
//! overwrites. A hold is the 200 ticks between the loops, and writes nothing
//! (`:194` only waits).
//!
//! One tick is one frame — one `dis_waitb` — so a screen is 363 ticks: the
//! parked one, the one the palette and the picture land on, the 102 open steps,
//! the 200 hold ticks and the 59 close steps. The `for(a=0;a<10000;a++)` delay
//! between each step's `tw_setsplit` and its `tw_setstart`
//! (`MAIN.C:182`, `:202`) is a hardware-timing artefact and is **not modelled**:
//! it exists to keep the writes inside the vertical retrace, and the port's tick
//! is already the frame boundary it protects. The shipped build replaces it with
//! `while(inp(0x3da)&1);` after the step's `dis_waitb` (`0x1c4d-0x1c5a`,
//! `0x1cfb-0x1d08`) and writes the start before the split; neither is modelled
//! either, and the record's 28 wipe frames are the port's states byte for byte
//! with the writes as they are (`docs/fidelity/cred.md` §3).
//!
//! **The release's screens are longer than 363 frames, and the port's are not.**
//! The record's wipe frames put each screen 3 to 4 frames later than the last
//! (`tests::the_records_transition_frames_are_the_states_this_part_writes`), all
//! of it between one screen's close and the next one's open — `screenin`'s
//! caption and picture, a far call to `tw_putpixel` for each of the picture's
//! 16,000 pixels and 32 for each caption column, cost the emulated CPU whole
//! frames. The image's loop counts are the tree's, so this is CPU time,
//! which the tick map does not model (A11, A53).
//!
//! # The three things worth knowing before changing anything here
//!
//! - **The font strip is not loaded through [`sr_assets::font`].** The routine
//!   reads 32 rows (`MAIN.C:29`) of a raster that has **40**, where a `Font`'s
//!   glyphs are every row of the strip. The routine *scans* for ink runs
//!   (`:250-269`) and indexes the raster by column, so the part reads the
//!   raster itself.
//!   `docs/fidelity/APPROXIMATIONS.md` A8.
//! - **`tw_setpalette` uploads 768 bytes of the picture's own palette**
//!   (`TWEAK.ASM:155-176`) into DAC entries 0-255, after `init`'s repair. The
//!   picture is drawn at `+16`, so picture pixel `v` shows the repaired entry
//!   `v+16`, which is the picture's own entry `v` — except for `v` 0-9, which
//!   the grey ramp overwrote.
//! - **`tw_clrscr` (`TWEAK.ASM:62-75`) clears 1,000h dwords**: 16,384 byte
//!   addresses × 4 planes = 65,536 pixels, not the whole surface. The picture
//!   band from the previous screen is deliberately left standing and is fully
//!   overwritten by this screen's, whose band is the same 160×100.

use sr_assets::exe::{unpack, LoadImage};
use sr_assets::{load, Format};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::{Split, View};
use sr_engine::{Palette, Surface};

use crate::parts::read_asset;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "cred";

// --------------------------------------------------------------- the image --

/// The shipped part, packed: PKLITE 1.14 large, one layer (`docs/fidelity/release-images.md`).
/// [`sr_assets::exe::unpack`] gives the load image DOS runs — 426,384 bytes, a Borland C++
/// large-model program — and every offset below is an offset into that image.
const EXE_PATH: &str = "assets/MAIN/DATA/CRED.EXE";
/// DGROUP's image offset: `mov dx,6752h` (`ba 52 67`) at image 0, ×16. The captions are near
/// pointers into it.
const DGROUP: usize = 0x6752 * 16;
/// `main` (`0x190c-0x1b68`, `MAIN.C:36-159`): from its first instruction, `push ds` before the
/// release's own `printf` (`0x190c-0x1915`, which the tree does not have), to its `lret`;
/// `screenin` begins at `0x1b69` — the `lcall 0190:0269` target.
const MAIN_AT: usize = 0x190c;
const MAIN_END: usize = 0x1b69;
/// One of `main`'s 21 `if(!dis_exit()) screenin(pic, text)` calls, after its `dis_exit` test:
/// `push ds; push text; push pic_seg; push 0; lcall 0190:0269` —
/// `1e 68 TT TT 68 SS SS 68 00 00 9a 69 02 90 01`. `pic` is the far pointer `SS SS:0000`, a
/// segment the loader relocates; `text` is the near offset `TT TT` into DGROUP (`ds`).
const SCREENIN_CALL: [Option<u8>; 15] = [
    Some(0x1e),
    Some(0x68),
    None,
    None,
    Some(0x68),
    None,
    None,
    Some(0x68),
    Some(0x00),
    Some(0x00),
    Some(0x9a),
    Some(0x69),
    Some(0x02),
    Some(0x90),
    Some(0x01),
];
/// Where the call's two words sit, from its first byte.
const SCREENIN_TEXT: usize = 2;
const SCREENIN_PICTURE: usize = 5;
/// `MAIN.C:41-156`: twenty-one screens, and the image's `main` has as many calls.
const SCREEN_COUNT: usize = 21;

// ---------------------------------------------------------------- the font --

/// `char far font[FONAY][1500]` (`MAIN.C:31`), in the image: `mov dx,029fh; mov es,dx` before
/// every read of it (`0x1e83`, `0x1ec5` in `init`'s scan, `0x1db0` in `prt`) — a far array at
/// segment `0x29f`, image `0x29f0`. It is the tree's `CREDITS/FONA.INC` byte for byte.
const FONT_AT: usize = 0x29f * 16;
/// The raster's row stride: `mov dx,05dch; imul dx` (`0x1e7b`, `0x1ebd`, `0x1da8`) — 1,500, as
/// `MAIN.C:31` declares it.
const FONT_WIDTH: usize = 1500;
/// The rows the image links: 40, the 60,000 bytes from [`FONT_AT`] to the first picture
/// (segment `0x1145`). `APPROXIMATIONS.md` A8.
const FONT_ARRAY_ROWS: usize = 40;
/// `#define FONAY 32` (`MAIN.C:29`): `cmp word [bp-4],20h` in the scan (`0x1e95`, `0x1ed7`) and
/// `y2w = y+20h` in `prt` (`0x1d60`). The rows the routine reads, of the 40 the image holds. The
/// eight it never reads hold 92 inked pixels, and none of them is in a column the first 32 rows
/// leave blank — so the metrics are the same either way, and only those 92 pixels can differ.
/// `APPROXIMATIONS.md` A8.
const FONT_ROWS: usize = 32;
/// The far pointer `fonaorder` (`MAIN.C:32`), at DGROUP `0x92`: `les bx,[0092h]` (`0x1eed`).
/// The string it points at (DGROUP `0x96`) is the tree's 47 characters.
const FONAORDER_PTR: usize = DGROUP + 0x92;
/// `fonap[32] = 1500-32` (`MAIN.C:270`; `mov word [07aeh],05bch` at `0x1f30`): the blank's
/// column.
const BLANK_COLUMN: u16 = (FONT_WIDTH - 32) as u16;
/// `fonaw[32] = 8` (`MAIN.C:271`; `mov word [09aeh],8` at `0x1f36`): the blank's width — the
/// routine's own, and not a run the scan can find (the blank's columns hold no ink).
const BLANK_WIDTH: u8 = 8;
/// The blank's character: `fonap[32]`/`fonaw[32]` are indexed by the string's
/// bytes, and 32 is the space.
const BLANK: u8 = b' ';

/// Where a caption line is centred horizontally (`MAIN.C:172`: `prtc(160,y,text)`;
/// `push 0a0h` at `0x1bc3`) and where its first line starts (`mov word [bp-6],10h`, `0x1baf`).
const TEXT_CENTRE: usize = 160;
const TEXT_FIRST_LINE: usize = 16;
/// `y += FONAY+10` (`MAIN.C:172`; `add word [bp-6],2ah` at `0x1bb6`): the step between caption
/// lines.
const TEXT_LINE_STEP: usize = FONT_ROWS + 10;

// ------------------------------------------------------------ the pictures --

/// The pictures' geometry: `screenin`'s `char (*pic)[160]`, and `:174`'s 160×100 loop
/// (`cmp word [bp-4],0a0h` at `0x1c29`, `cmp word [bp-6],64h` at `0x1c20`).
const PICTURE_WIDTH: usize = 160;
const PICTURE_HEIGHT: usize = 100;
/// One linked picture, an `FC FC` blob: a 16-byte header (160×100, 256 colours, pixels at
/// paragraph 49), the 768-byte palette at 16 (`tw_setpalette(&pic[0][16])`, `0x1b8d-0x1b97`) and
/// the pixels at 784 (`pic = &pic[0][784]`, `add dx,310h` at `0x1ba5`): 16,784 bytes, the
/// 0x419-paragraph step between the 21 segments `main` pushes.
const PICTURE_BYTES: usize = 784 + PICTURE_WIDTH * PICTURE_HEIGHT;
/// `:174`'s `pic[y][x]+16`: the picture's colours are 16 entries into the
/// screen's palette.
const PICTURE_COLOUR_OFFSET: u8 = 16;
/// `:174`'s own coordinates: the picture starts at pen pixel (400, 400), and
/// each of its rows is two logical lines below the last.
const PICTURE_FIRST_COLUMN: usize = 400;
const PICTURE_FIRST_LINE: usize = 400;
const PICTURE_LINE_STEP: usize = 2;

/// `init`'s palette repair (`MAIN.C:273-317`): the `memmove`'s extent and
/// destination, and the ramp's.
const REPAIR_COUNT: usize = 240;
const REPAIR_SHIFT: u8 = 16;
const RAMP_ENTRIES: u8 = 10;
const RAMP_STEP: u8 = 7;

// ------------------------------------------------------------- the screen --

/// The pen's logical line, in pixels: `tw_putpixel`'s `y*80 + x/4` byte address
/// holds four pixels, one per plane (`TWEAK.ASM:104-110`). The number the part
/// writes *flat* with, and half the view's stride.
const LINE_LEN: usize = 320;

/// `tw_clrscr` (`TWEAK.ASM:62-75`): `mov cx,1000h` / `rep stosd` with the map
/// mask open is 4,096 dword writes = 16,384 byte addresses, and each address
/// carries four pixels — one per plane — so 65,536 pixels.
const CLRSCR_PIXELS: usize = 16_384 * 4;

/// The displayed geometry: 320 pixels a row (`TWEAK.ASM:43-44`, CRTC
/// 0x13 = `0x5013`) and 200 rows (`:46-47`, CRTC 9 = `0x0109`).
const VIEW_WIDTH: u16 = 320;
const VIEW_HEIGHT: u16 = 200;
/// CRTC 0x13's 80 words = 160 displayed bytes a row = 640 pixels.
const DISPLAY_LINE_STRIDE: u16 = 640;

/// `tw_setstart(160*200)` (`MAIN.C:167`): 32,000 bytes = 400 logical lines =
/// 128,000 pixels. The parked start — the *even* logical lines.
const PARKED_ORIGIN: u32 = 160 * 200 * 4;
/// `tw_setsplit(400)` (`MAIN.C:165`): a line compare below a 200-row display,
/// so the parked split is inert.
const PARKED_LINE_COMPARE: u16 = 400;

/// The state every frame of a hold shows: what the open loop's **last** step
/// leaves.
///
/// `MAIN.C:176`'s last open step is `y = 1`, the smallest value `y*12L/13`
/// reaches before 0, and `:182` writes `tw_setstart(160*200 + yy/4)` with
/// `yy = 320 - y/80 = 320`: 32,000 + 80 bytes = **32,080** = logical line
/// **401** = 400 × 320 + 320 = 128,320 pixels. That is why the *odd* logical
/// lines — the ones the picture is drawn into — are the ones displayed, and why
/// one displayed row is two logical lines. One step's worth of scroll is one
/// logical line, which is exactly [`LINE_LEN`].
///
/// This is **not a fitted constant**: it is the arithmetic of the source's own
/// last step, and it is checked against the record by
/// `the_holds_display_state_reproduces_the_references_own_frames`, where the
/// port's screen 1 and screen 18 are `reference/parts/cred`'s frames at 1440
/// and 7640 byte for byte. At the parked origin those frames differ in every
/// picture pixel; one logical line is the whole difference.
///
/// **The part's own code no longer names these three**: the open loop's last
/// step is what writes them, and [`Display::opening`] reads that step's own
/// `y`. They are the tests' copies of the arithmetic, so that a ramp which
/// stopped producing them fails in a place that says which number moved.
#[cfg(test)]
const HOLD_ORIGIN: u32 = PARKED_ORIGIN + LINE_LEN as u32;
/// The hold's line compare (`MAIN.C:179`: `y/128 + 200` with `y = 1`): 200.
#[cfg(test)]
const HOLD_LINE_COMPARE: u16 = 200;
/// The pan the hold shows (`:183-191` with `yy = 320`): `(320 & 3) * 2 = 0`
/// register counts, and so 0 pixels ([`pan`]).
#[cfg(test)]
const HOLD_PAN: u8 = 0;

/// Where `tw_setstart`'s argument starts counting from (`MAIN.C:167`, `:182`,
/// `:202`): `160*200` bytes, the parked start.
const DISPLAY_START_BIAS: u16 = 160 * 200;
/// `tw_setsplit`'s (`:179`, `:199`): `y/128 + 200`.
const LINE_COMPARE_BIAS: u16 = 200;

/// The port's split for a line-compare value: the first row *below* the split.
///
/// The hardware's register names the last line above it, so that line plus one
/// — and because the display is double-scanned and the port's view folds that
/// into the stride, one displayed row is two scanlines, so the emulator's own
/// `(line_compare + 1) / lines_scaled` (`vga_draw.cpp:1586-1593`, with
/// `lines_scaled` 2 for a double-scanned mode) is `(line_compare + 1) / 2` here.
///
/// Measured, not read: `tw_setsplit(200)` puts the caption's first line on the
/// lower view's *first* row, and the port's screen 1 — which has that split —
/// reproduces the reference's frame 1440 exactly; one row out is 320 pixels of
/// difference there. `tw_setsplit(400)` is then row `(400+1)/2 = 200`, the
/// first row below a 200-row display: inert, which is what "parked" means.
const fn split_scanline(line_compare: u16) -> u16 {
    line_compare.div_ceil(2)
}

/// One iris step's three register writes, in the port's own units.
///
/// `MAIN.C:179-191` (the open loop) and `:199-211` (the close) are the same
/// three writes, from the step's `y` and the `yy` it makes of it:
///
/// ```text
/// tw_setsplit(y/128 + 200)          the line compare
/// yy = 320 - y/80     (opening)     and the two registers below
/// yy = 320 + y/80     (closing)
/// tw_setstart(160*200 + yy/4)       the display start, in bytes
/// AC 0x13 = (yy & 3) * 2            the pan
/// ```
///
/// `tw_setstart` names a **byte** address (CRTC 0x17 = `0xE3` has its byte-mode
/// bit set, `TWEAK.ASM:40-41`) and a byte address of this mode is four pixels —
/// one per plane, [`LINE_LEN`]'s own arithmetic — so the start is the byte
/// address times four in the port's pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Display {
    /// The display start, in the port's pixels.
    origin: u32,
    /// The line-compare register, as `tw_setsplit` is given it.
    line_compare: u16,
    /// The pan in the port's pixels — *not* the AC 0x13 value; see [`pan`].
    pan: u8,
}

impl Display {
    /// `MAIN.C:165-167`: `tw_setsplit(400)`, `tw_setstart(160*200)`, no pan.
    const PARKED: Display = Display {
        origin: PARKED_ORIGIN,
        line_compare: PARKED_LINE_COMPARE,
        pan: 0,
    };

    /// One step of the open loop (`:179-191`), whose `yy` rises from 0 to 320
    /// as `y` falls from 200*128 to 1.
    const fn opening(y: u16) -> Display {
        Display::from_y(y, 320 - y / 80)
    }

    /// One step of the close loop (`:199-211`), whose `yy` rises from 320 to
    /// 629 as `y` rises from 0 to 24,795.
    const fn closing(y: u16) -> Display {
        Display::from_y(y, 320 + y / 80)
    }

    /// The three registers both loops write from one step's `(y, yy)`.
    const fn from_y(y: u16, yy: u16) -> Display {
        Display {
            origin: (DISPLAY_START_BIAS + yy / 4) as u32 * 4,
            line_compare: y / 128 + LINE_COMPARE_BIAS,
            pan: pan(yy),
        }
    }
}

/// AC 0x13's effect, in the port's pixels.
///
/// `MAIN.C:183-191` writes `(yy & 3) * 2` to the register. The register and the
/// shift are two different numbers: the emulator's `case M_VGA: case M_LIN8:`
/// arm is `vga.config.pel_panning = (val & 0x7) / 2` (`vga_attr.cpp:225-227`),
/// so a 256-colour mode shifts half of what the register says. And the unit it
/// is half of is the display start's own: `vga_draw.cpp:1648-1651` builds a
/// 256-colour line's address as the CRTC's start times four, **plus** the pan,
/// so a pan of `n` reads `n` pixels further along — what [`View::pan`] does.
/// A step's pan is therefore `yy & 3` pixels, and writing the register value
/// here would move the display twice as far as the machine did.
const fn pan(yy: u16) -> u8 {
    (yy & 3) as u8
}

// ------------------------------------------------------------- the timing --

/// `MAIN.C:176`'s open loop as the sequence of `y` values it runs with:
/// `for(y=200*128; y>0; y=y*12L/13)`.
///
/// The port carries both ramps as the source's own integer sequences rather
/// than as closed forms that "match" them: a closed form a step out moves every
/// hold after it, and the holds are where the reference's keyframes are. This
/// task uses their **lengths**; Task 8 reads the values and writes the three
/// registers per step.
fn open_ramp() -> Vec<u16> {
    let mut y: u32 = 200 * 128;
    let mut steps = Vec::new();
    while y > 0 {
        steps.push(y as u16);
        y = y * 12 / 13;
    }
    steps
}

/// `MAIN.C:196`'s close loop: `for(y=0,v=0; y<128*200; y=y+v,v+=15)`. `y` is
/// what `tw_setsplit` is given and `yy = 320 + y/80` what `tw_setstart` is, so
/// both are carried and Task 8 does not have to re-derive the second.
fn close_ramp() -> Vec<(u16, u16)> {
    let (mut y, mut v): (u32, u32) = (0, 0);
    let mut steps = Vec::new();
    while y < 128 * 200 {
        steps.push((y as u16, v as u16));
        y += v;
        v += 15;
    }
    steps
}

/// `MAIN.C:165-168`: the tick the parked split, the clear and the parked start
/// are shown on, before the blank.
const PARKED_TICK: u32 = 0;
/// `MAIN.C:169-174`: the tick the palette, the caption and the picture land on.
const SHOW_TICK: u32 = PARKED_TICK + 1;
/// `MAIN.C:176-192`'s open loop and `:196-212`'s close loop, as the number of
/// `dis_waitb()`s the source's own arithmetic produces: 102 and 59.
///
/// The part's *timing* is computed from the loops themselves ([`Cred::screen_ticks`]),
/// so these two are the tests' — they are what
/// `the_wipes_ramps_are_the_sources_own_sequences` checks the loops against, and
/// a loop whose count drifted would fail there rather than silently moving every
/// screen after it.
#[cfg(test)]
const OPEN_STEPS: u32 = 102;
#[cfg(test)]
const CLOSE_STEPS: u32 = 59;
/// `MAIN.C:194`'s `for(a=0;a<200 && !dis_exit();a++) dis_waitb()`.
const HOLD_FRAMES: u32 = 200;

/// One screen: its picture, the palette the screen uploads, and its caption.
struct Screen {
    /// The picture's pixels, row-major at [`PICTURE_WIDTH`].
    picture: Vec<u8>,
    /// The picture's palette *after* `init`'s repair — what `tw_setpalette`
    /// uploads.
    palette: Palette,
    /// The caption's lines, in the image's order. A line is one of the NUL-terminated runs at
    /// the call's `text`, so an empty entry is a line that draws nothing and still advances `y`
    /// (`:172`).
    text: Vec<Vec<u8>>,
}

/// One of `main`'s `screenin` calls, as the image holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenCall {
    /// The call's image offset: its `push ds`.
    at: usize,
    /// `pic`'s segment, relative to the load segment: the picture is at image `segment * 16`.
    segment: u16,
    /// `text`'s offset in DGROUP: the caption is at image `DGROUP + text`.
    text: u16,
}

/// What `init` and `main` read out of the image: the font raster (all [`FONT_ARRAY_ROWS`] rows),
/// `fonaorder` (the bytes up to its NUL), and the 21 calls with each call's picture blob and
/// caption lines.
struct ImageData {
    raster: Vec<u8>,
    order: Vec<u8>,
    calls: Vec<ScreenCall>,
    pictures: Vec<Vec<u8>>,
    captions: Vec<Vec<Vec<u8>>>,
}

/// `main`'s `screenin` calls, in the image's order: every match of [`SCREENIN_CALL`] inside
/// `main`.
fn screenin_calls(image: &[u8]) -> Vec<ScreenCall> {
    let word = |at: usize| u16::from_le_bytes([image[at], image[at + 1]]);
    let end = MAIN_END.min(image.len());
    (MAIN_AT..end.saturating_sub(SCREENIN_CALL.len() - 1))
        .filter(|&at| {
            SCREENIN_CALL
                .iter()
                .zip(&image[at..at + SCREENIN_CALL.len()])
                .all(|(want, byte)| want.is_none_or(|want| want == *byte))
        })
        .map(|at| ScreenCall {
            at,
            segment: word(at + SCREENIN_PICTURE),
            text: word(at + SCREENIN_TEXT),
        })
        .collect()
}

/// The caption at `at`: `MAIN.C:172`'s `while(*(text=prtc(160,y,text)))`. `prtc` draws the line
/// up to its NUL and returns the byte after it, and the loop stops when that byte is NUL — so the
/// first line is drawn even when it is empty, and the caption ends at a double NUL.
fn caption_lines(image: &[u8], mut at: usize) -> Result<Vec<Vec<u8>>, String> {
    let mut lines = Vec::new();
    loop {
        let tail = image.get(at..).unwrap_or_default();
        let len = tail
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| format!("the caption line at {at:#x} has no NUL"))?;
        lines.push(tail[..len].to_vec());
        at += len + 1;
        match image.get(at) {
            Some(0) => return Ok(lines),
            Some(_) => {}
            None => return Err(format!("the caption runs off the image at {at:#x}")),
        }
    }
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
    if bytes.len() < MAIN_END {
        return Err(format!(
            "the image is {} bytes; main ends at {MAIN_END:#x}",
            bytes.len()
        ));
    }
    let calls = screenin_calls(bytes);
    if calls.len() != SCREEN_COUNT {
        return Err(format!(
            "main has {} screenin calls where this build has {SCREEN_COUNT}",
            calls.len()
        ));
    }
    // The picture's segment is a relocated word, and the caption's offset is not: a call whose
    // relocations say otherwise is not the call this pattern means.
    for call in &calls {
        let relocated = |at: usize| image.relocations.contains(&(at as u32));
        if !relocated(call.at + SCREENIN_PICTURE) || relocated(call.at + SCREENIN_TEXT) {
            return Err(format!(
                "the screenin call at {:#x} is not relocated as a far pic",
                call.at
            ));
        }
    }

    // The far pointer `fonaorder`, as the image holds it before relocation: the segment word is
    // relative to the load segment, which is the image's own origin.
    let order_at = word(FONAORDER_PTR + 2)? * 16 + word(FONAORDER_PTR)?;
    let order_len = bytes
        .get(order_at..)
        .and_then(|tail| tail.iter().position(|&byte| byte == 0))
        .ok_or_else(|| format!("fonaorder at {order_at:#x} has no terminator"))?;
    let raster_end = FONT_AT + FONT_ARRAY_ROWS * FONT_WIDTH;
    let first_picture = calls
        .iter()
        .map(|call| usize::from(call.segment) * 16)
        .min();
    if first_picture != Some(raster_end) {
        return Err(format!(
            "the raster ends at {raster_end:#x}, and the first picture is at {first_picture:x?}"
        ));
    }

    let mut read = vec![(FONT_AT, raster_end), (order_at, order_at + order_len)];
    let mut pictures = Vec::with_capacity(SCREEN_COUNT);
    let mut captions = Vec::with_capacity(SCREEN_COUNT);
    for call in &calls {
        let at = usize::from(call.segment) * 16;
        let blob = bytes
            .get(at..at + PICTURE_BYTES)
            .ok_or_else(|| format!("the picture at {at:#x} runs off the image"))?;
        pictures.push(blob.to_vec());
        read.push((at, at + PICTURE_BYTES));
        let text = DGROUP + usize::from(call.text);
        let lines = caption_lines(bytes, text)?;
        let len: usize = lines.iter().map(|line| line.len() + 1).sum::<usize>() + 1;
        read.push((text, text + len));
        captions.push(lines);
    }
    // A relocation inside what is read would be a segment word, not data.
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
        calls,
        pictures,
        captions,
    })
}

/// CRED: the strip, the 21 screens, and where in a screen the part's tick is.
pub struct Cred {
    /// The font raster, row-major at [`FONT_WIDTH`] — all 40 rows of the image's
    /// array, of which the routine reads [`FONT_ROWS`].
    strip: Vec<u8>,
    /// `fonaorder` (`MAIN.C:32`), as the image holds it.
    order: Vec<u8>,
    /// `fonap` (`MAIN.C:33`): the column each character's ink starts at.
    fonap: [u16; 256],
    /// `fonaw` (`MAIN.C:34`): each character's ink width, and the routine's 8
    /// for the blank.
    fonaw: [u8; 256],
    /// The 21 screens in `main`'s order. Empty until `init`.
    screens: Vec<Screen>,
    /// `MAIN.C:176`'s loop, as the sequence of `y` values it runs with. The
    /// part's timing comes from its length, and Task 8's register writes from
    /// its values.
    open: Vec<u16>,
    /// `MAIN.C:196`'s loop, as (its `y`, its `v`) pairs.
    close: Vec<(u16, u16)>,
    /// Which screen this tick belongs to.
    screen: usize,
    /// The tick within that screen.
    tick: u32,
}

impl Cred {
    /// A part with nothing loaded: `init` is what unpacks the image and reads
    /// the strip and the 21 screens out of it.
    pub fn new() -> Cred {
        Cred {
            strip: Vec::new(),
            order: Vec::new(),
            fonap: [0; 256],
            fonaw: [0; 256],
            screens: Vec::new(),
            open: open_ramp(),
            close: close_ramp(),
            screen: 0,
            tick: 0,
        }
    }

    /// The ticks one screen takes, from the source's own three numbers: the
    /// blank tick, the palette-and-picture tick, the open loop's length, the
    /// hold, and the close loop's.
    fn screen_ticks(&self) -> u32 {
        SHOW_TICK + 1 + self.open.len() as u32 + HOLD_FRAMES + self.close.len() as u32
    }

    /// How many ticks the part runs in all: 21 screens of [`Cred::screen_ticks`].
    #[cfg(test)]
    fn total_ticks(&self) -> u32 {
        self.screen_ticks() * SCREEN_COUNT as u32
    }

    /// `MAIN.C:169-174` — the palette, the caption, then the picture.
    fn show_screen(&self, ctx: &mut PartCtx) {
        let screen = &self.screens[self.screen];
        ctx.set_palette(screen.palette);
        self.draw_text(ctx, &screen.text);
        self.draw_picture(ctx, &screen.picture);
    }

    /// `MAIN.C:172` — one centred line per entry of `text`, `FONAY+10` logical
    /// lines apart. The source's `while(*(text = prtc(...)))` stops at the
    /// terminating empty literal, which draws nothing and does not advance `y`,
    /// so the entries here are exactly the lines it draws.
    fn draw_text(&self, ctx: &mut PartCtx, text: &[Vec<u8>]) {
        let mut y = TEXT_FIRST_LINE;
        for line in text {
            self.draw_centred(ctx, TEXT_CENTRE, y, line);
            y += TEXT_LINE_STEP;
        }
    }

    /// `MAIN.C:236-243` — `prtc`: the line's width is `Σ(fonaw[ch]+2)`, and it
    /// is drawn from `x - w/2`. The sum is integers and the divide is at the
    /// end, so a line of odd width starts one pixel left of centre, as in the
    /// original.
    fn draw_centred(&self, ctx: &mut PartCtx, x: usize, y: usize, text: &[u8]) {
        let width: usize = text.iter().map(|&ch| usize::from(self.width(ch)) + 2).sum();
        self.draw_line(ctx, x - width / 2, y, text);
    }
    /// `MAIN.C:215-234` — `prt`: for each character, `fonaw[ch]` columns
    /// starting at `fonap[ch]`, `FONAY` rows each, then a two-column gap.
    fn draw_line(&self, ctx: &mut PartCtx, mut x: usize, y: usize, text: &[u8]) {
        for &ch in text {
            let width = usize::from(self.width(ch));
            let columns = usize::from(self.column(ch))..;
            for (column, at) in columns.zip(x..x + width) {
                for row in 0..FONT_ROWS {
                    let value = self.strip[row * FONT_WIDTH + column];
                    put(ctx.surface, at, y + row, value);
                }
            }
            x += width + 2;
        }
    }

    /// `fonaw[ch]`, with the blank's own entry (`MAIN.C:269-271`).
    fn width(&self, ch: u8) -> u8 {
        if ch == BLANK {
            BLANK_WIDTH
        } else {
            self.fonaw[usize::from(ch)]
        }
    }

    /// `fonap[ch]`, with the blank's own column (`MAIN.C:270`).
    fn column(&self, ch: u8) -> u16 {
        if ch == BLANK {
            BLANK_COLUMN
        } else {
            self.fonap[usize::from(ch)]
        }
    }

    /// `MAIN.C:174` — `for(x=0;x<160;x++) for(y=0;y<100;y++)
    /// tw_putpixel(400+x,400+y*2,pic[y][x]+16)`.
    ///
    /// The pen's address is `y*80 + x/4` in plane `x & 3`, which in the port's
    /// flat pixel units is exactly `320*y + x` — four pixels a byte, one plane
    /// apiece — so this is a flat write at the coordinates the source names.
    /// The source's loop order is x-outer, and it is kept: the two are disjoint
    /// here, but the order is the source's and costs nothing.
    fn draw_picture(&self, ctx: &mut PartCtx, picture: &[u8]) {
        for x in 0..PICTURE_WIDTH {
            for y in 0..PICTURE_HEIGHT {
                let value = picture[y * PICTURE_WIDTH + x];
                put(
                    ctx.surface,
                    PICTURE_FIRST_COLUMN + x,
                    PICTURE_FIRST_LINE + PICTURE_LINE_STEP * y,
                    value.wrapping_add(PICTURE_COLOUR_OFFSET),
                );
            }
        }
    }

    /// The display, as the view the port holds: one step's three register
    /// writes (`MAIN.C:179-191`), or the parked state (`:165-167`).
    fn set_display(ctx: &mut PartCtx, display: Display) {
        // `TWEAK.ASM:18-60`: 320 pixels a row, 200 rows, a 160-byte line offset
        // and no separate scan doubling — the doubling the CRTC asks for is
        // what the 160-byte offset already folds in, which is the arithmetic
        // this module's docs spell out.
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.y_line_offset = Some(DISPLAY_LINE_STRIDE);
        ctx.view.double_scan = false;
        ctx.view.pan = display.pan;
        ctx.view.set_origin(display.origin);
        // The lower view reads from origin 0 with the surface's own stride
        // (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`): the hardware sets the
        // address counter to 0 and clears panning at the split, and the line
        // offset register is unchanged — 160 bytes, the same 640 pixels the
        // surface's own rows are.
        ctx.view.split = Some(Box::new(Split {
            scanline: split_scanline(display.line_compare),
            view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
        }));
    }

    /// The three registers the iris writes at tick `tick` of a screen, or
    /// `None` at a tick that writes none — the hold (`MAIN.C:194`, which only
    /// waits).
    ///
    /// The ticks a screen's wipes occupy, from the source's own structure: the
    /// `dis_waitb` at `:168` ends the tick the palette and the picture land on
    /// ([`SHOW_TICK`]), so the open loop's first step is the next tick and its
    /// 102nd is tick `SHOW_TICK + 102` — the frame the hold's state is first
    /// displayed on, and why the hold's own 200 frames start one tick later.
    /// The close loop's 59 steps follow it.
    fn iris_display(&self, tick: u32) -> Option<Display> {
        let step = tick.checked_sub(SHOW_TICK + 1)? as usize;
        if step < self.open.len() {
            return Some(Display::opening(self.open[step]));
        }
        let close = step.checked_sub(self.open.len() + HOLD_FRAMES as usize)?;
        self.close.get(close).map(|(y, _)| Display::closing(*y))
    }
}

impl Part for Cred {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:38-39`'s `init()` and `tw_opengraph`, plus the font and the 21
    /// screens, read out of the shipped image.
    ///
    /// The metrics scan (`:250-269`), the pictures with their palettes repaired
    /// (`:273-317`), and `tw_opengraph`'s clear (`TWEAK.ASM:29-34`) — the whole
    /// 256 KB, so the port's whole surface. The CRTC and AC state
    /// `tw_opengraph` writes is the view, which is set per screen.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let error = |message: String| PartError {
            part: NAME,
            message: format!("{EXE_PATH}: {message}"),
        };
        let file = read_asset(NAME, EXE_PATH)?;
        let image = unpack(&file).map_err(|e| error(e.to_string()))?;
        let data = image_data(&image).map_err(error)?;
        self.strip = data.raster;
        self.order = data.order;
        scan_metrics(&self.strip, &self.order, &mut self.fonap, &mut self.fonaw);

        self.screens = Vec::with_capacity(SCREEN_COUNT);
        for ((call, blob), text) in data.calls.iter().zip(&data.pictures).zip(data.captions) {
            let at = usize::from(call.segment) * 16;
            let picture = load(blob, Format::Origin)
                .map_err(|e| error(format!("the picture at {at:#x}: {e}")))?;
            let (width, height) = (picture.width() as usize, picture.height() as usize);
            if (width, height) != (PICTURE_WIDTH, PICTURE_HEIGHT) {
                return Err(error(format!(
                    "the picture at {at:#x} is {width}x{height}; this part's pictures are \
                     {PICTURE_WIDTH}x{PICTURE_HEIGHT}"
                )));
            }
            // `MAIN.C:273-317` (`0x1f3c-0x2712`): `init` repairs the 21 palettes the calls
            // upload, in place.
            let mut palette = picture.palette;
            repair_palette(&mut palette);
            self.screens.push(Screen {
                picture: picture.pixels().to_vec(),
                palette,
                text,
            });
        }

        // `TWEAK.ASM:29-34`: 32,768 word writes with the map mask open, which
        // is 65,536 byte addresses in each of the four planes — every pixel the
        // machine has, and so the whole surface. Nothing a previous part left
        // can show through a credit screen.
        ctx.surface.clear(0);
        Cred::set_display(ctx, Display::PARKED);
        Ok(())
    }

    /// One tick of one screen. `MAIN.C:161-213`'s shape, one tick a vertical
    /// blank:
    ///
    /// - tick 0 (`:165-168`) — the parked split, `tw_clrscr`, the parked start,
    ///   then the blank.
    /// - tick 1 (`:169-174`) — the palette, the caption, the picture.
    /// - the next 102 ticks — one per step of [`open_ramp`] (`:176-192`) — the
    ///   iris opening, one step's three registers each.
    /// - then [`HOLD_FRAMES`] ticks (`:194`) — the hold, with nothing written.
    /// - then 59 ticks — one per step of [`close_ramp`] (`:196-212`) — the iris
    ///   closing, after which the next screen's tick 0 re-parks the display.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let tick = self.tick;
        if tick >= self.screen_ticks() {
            return PartStatus::Finished;
        }
        match tick {
            PARKED_TICK => {
                Cred::set_display(ctx, Display::PARKED);
                // `TWEAK.ASM:62-75`: 1,000h dwords = 65,536 pixels, *not* the
                // whole surface — the picture band is past the clear and is
                // overwritten by this screen's own picture.
                for pixel in &mut ctx.surface.pixels_mut()[..CLRSCR_PIXELS] {
                    *pixel = 0;
                }
            }
            SHOW_TICK => self.show_screen(ctx),
            _ => {
                // The open loop's steps, then nothing through the hold, then
                // the close loop's: [`Cred::iris_display`] is where each step
                // of each loop lands, and the three registers it writes are the
                // step's own.
                if let Some(display) = self.iris_display(tick) {
                    Cred::set_display(ctx, display);
                }
            }
        }

        self.tick += 1;
        if self.tick == self.screen_ticks() {
            self.tick = 0;
            self.screen += 1;
            if self.screen == self.screens.len() {
                // `MAIN.C:158-159`: the epilogue is commented out and `:159`'s
                // `}` ends `main()`. The loader holds the screen it left.
                return PartStatus::Finished;
            }
        }
        PartStatus::Continuing
    }
}

/// The pen: `tw_putpixel(x, y, c)` writes plane `x & 3`, byte address
/// `y*80 + x/4` (`TWEAK.ASM:87-117`). Four pixels to the byte, one plane
/// apiece, so flat pixel `4*(y*80 + x/4) + (x & 3)` is `320*y + x` — exactly
/// [`LINE_LEN`]'s line.
///
/// The source's address counter is 16 bits and wraps at 65,536 bytes = 262,144
/// pixels. The port's surface is 256,000 — the whole display canvas — and no
/// write this part makes comes near either bound: the largest is the last
/// picture pixel, `320*598 + 400 + 159 = 191,919`.
fn put(surface: &mut Surface, x: usize, y: usize, value: u8) {
    let at = y * LINE_LEN + x;
    if at < surface.pixels().len() {
        surface.pixels_mut()[at] = value;
    }
}

/// `MAIN.C:273-317` — the repair `init` does to each picture's palette, in the
/// source's own two moves.
///
/// 1. `memmove(&pic[16*3+16], &pic[16], 768-16*3)` (`:273-293`): 720 bytes,
///    palette entries 0-239 → 16-255. The 768-byte palette sits at offset 16 of
///    the picture's entry, so destination offset 64 is entry 16 and the move
///    covers 240 entries. Entries 0-15 are left standing, which is why the same
///    source colour can be reached through two entries — `v` and `v+16` for `v`
///    ≤ 15 — and why the port snapshots first, which is `memmove`'s
///    overlapping-copy semantics.
/// 2. The ten-entry grey ramp (`:294-317`): `pic[a*3+k+16] = 7*a` for `a` 0-9 —
///    palette entries **0-9**, the *low* ones, because the palette starts at
///    offset 16 of the picture. That is what gives the captions their greys:
///    the text is drawn with the strip's own values 0-9, so it shows the ramp,
///    while the picture is drawn at `+16` and shows entry `v+16`, its own colour
///    `v`.
fn repair_palette(palette: &mut Palette) {
    let source: [[u8; 3]; REPAIR_COUNT] = core::array::from_fn(|i| palette.entry(i as u8));
    for (i, rgb) in source.iter().enumerate() {
        palette.set_entry(i as u8 + REPAIR_SHIFT, *rgb);
    }
    for a in 0..RAMP_ENTRIES {
        palette.set_entry(a, [RAMP_STEP * a, RAMP_STEP * a, RAMP_STEP * a]);
    }
}

/// `MAIN.C:250-269` — the scan that turns the strip into `fonap`/`fonaw`.
///
/// The outer loop walks `x` to 1,500 or until the order runs out; each pass
/// skips blank columns and takes the run of inked ones as the next character's
/// cell. A character the strip has no run for keeps the table's zero, which is
/// what a static array does in the original too; this strip holds one run per
/// character, so no character is left out.
fn scan_metrics(strip: &[u8], order: &[u8], fonap: &mut [u16; 256], fonaw: &mut [u8; 256]) {
    let inked = |x: usize| (0..FONT_ROWS).any(|y| strip[y * FONT_WIDTH + x] != 0);
    let mut x = 0usize;
    for &ch in order {
        while x < FONT_WIDTH && !inked(x) {
            x += 1;
        }
        if x >= FONT_WIDTH {
            break;
        }
        let start = x;
        while x < FONT_WIDTH && inked(x) {
            x += 1;
        }
        fonap[usize::from(ch)] = start as u16;
        fonaw[usize::from(ch)] = (x - start) as u8;
    }
    fonap[usize::from(BLANK)] = BLANK_COLUMN;
    fonaw[usize::from(BLANK)] = BLANK_WIDTH;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{differing_bytes, reference, Rig};
    use crate::shell::SURFACE_WIDTH;
    use std::path::Path;

    /// Read the tree's `FONA.INC` into its raster: the strip as the tree stored it.
    ///
    /// The part reads its raster out of the shipped image; this reader is the tests', for the
    /// measurement that says the image's raster is this file byte for byte.
    ///
    /// The file is not a container and has no magic — it is assembler source, one
    /// `db` line of comma-separated values after another — and the port reads it
    /// rather than a converted copy because the file *is* the asset the part
    /// linked (`CREDITS/INCLUDE.ASM`'s `asdf` segment, and byte for byte the raster
    /// `CREDITS/INCLUDE.OBJ` carries). The geometry is the source's: `MAIN.C:31`
    /// declares the symbol `[FONAY][1500]`, so the width is 1,500 and the values
    /// are read in order.
    ///
    /// A line that is neither blank, a comment, nor `db` is refused rather than
    /// skipped: any other construct the file could grow is one this reader would
    /// otherwise read as an absent value.
    fn parse_strip(path: &Path) -> Result<Vec<u8>, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        let mut values = Vec::new();
        for (number, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            let Some(rest) = strip_db(line) else {
                return Err(format!("line {}: {line:?} is not a `db` line", number + 1));
            };
            for field in rest.split(',') {
                let field = field.trim();
                if field.is_empty() {
                    continue;
                }
                let value: u16 = field
                    .parse()
                    .map_err(|_| format!("line {}: {field:?} is not a number", number + 1))?;
                if value > u8::MAX.into() {
                    return Err(format!("line {}: {value} is not a byte", number + 1));
                }
                values.push(value as u8);
            }
        }
        if values.is_empty() || values.len() % FONT_WIDTH != 0 {
            return Err(format!(
                "{} values is not a whole number of {FONT_WIDTH}-column raster rows",
                values.len()
            ));
        }
        Ok(values)
    }

    /// The body of a `db` line, or `None` when the line is something else. The tag
    /// is matched case-insensitively, which is how the file spells it.
    fn strip_db(line: &str) -> Option<&str> {
        if line.len() < 2 {
            return None;
        }
        if !line[..2].eq_ignore_ascii_case("db") {
            return None;
        }
        Some(&line[2..])
    }

    /// The tree's own font and pictures, which the part no longer reads: the tests compare the
    /// image's data with them, to measure what the release changed.
    const TREE_FONT: &str = "../../assets/CREDITS/FONA.INC";
    const TREE_PICTURES: &str = "../../assets/CREDITS/PICS";
    /// `MAIN.C:32`'s `fonaorder`.
    const TREE_FONT_ORDER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/?!:,.\"()+-";

    /// The tree's 21 screens, in `MAIN.C:41-156`'s order: the picture's file and the caption's
    /// lines. This was the port's table before the image was read; it is kept to measure the
    /// release against.
    const TREE_SCREENS: [(&str, &[&str]); 21] = [
        (
            "PIC01.LBM",
            &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - WILDFIRE"],
        ),
        (
            "PIC02.LBM",
            &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - PSI"],
        ),
        (
            "PIC03.LBM",
            &[
                "GRAPHICS - MARVEL",
                "MUSIC - SKAVEN",
                "CODE - WILDFIRE",
                "ANIMATION - TRUG",
            ],
        ),
        ("PIC04.LBM", &["", "GRAPHICS - PIXEL"]),
        (
            "PIC05.LBM",
            &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        ("PIC05B.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - TRUG"]),
        ("PIC06.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
        ("PIC07.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
        (
            "PIC08.LBM",
            &["", "GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION"],
        ),
        (
            "PIC09.LBM",
            &[
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - TRUG",
                "RENDERING - TRUG",
            ],
        ),
        (
            "PIC10.LBM",
            &[
                "GRAPHICS - PIXEL, SKAVEN",
                "MUSIC - PURPLE MOTION",
                "CODE - PSI",
            ],
        ),
        (
            "PIC10B.LBM",
            &[
                "GRAPHICS - PIXEL, SKAVEN",
                "MUSIC - PURPLE MOTION",
                "CODE - PSI",
            ],
        ),
        (
            "PIC11.LBM",
            &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"],
        ),
        (
            "PIC12.LBM",
            &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"],
        ),
        ("PIC13.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
        (
            "PIC14.LBM",
            &[
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - TRUG",
                "RENDERING - TRUG",
            ],
        ),
        ("PIC14B.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
        (
            "PIC15.LBM",
            &["GRAPHICS - MARVEL", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        ("PIC16.LBM", &["", "MUSIC - SKAVEN", "CODE - PSI"]),
        (
            "PIC17.LBM",
            &["", "GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION"],
        ),
        (
            "PIC18.LBM",
            &[
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - WILDFIRE",
            ],
        ),
    ];

    /// The unpacked image, as the part reads it.
    fn image() -> LoadImage {
        unpack(&read_asset(NAME, EXE_PATH).unwrap()).unwrap()
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Cred) {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT);
        let mut part = Cred::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Cred, ticks: u32) {
        for at in 0..ticks {
            assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Continuing, "tick {at}");
        }
    }

    /// The width of a caption line, by the source's own arithmetic.
    fn line_width(part: &Cred, line: &[u8]) -> usize {
        line.iter().map(|&ch| usize::from(part.width(ch)) + 2).sum()
    }

    #[test]
    fn the_strip_parses_to_the_raster_the_source_declares() {
        // `MAIN.C:31`: `char far font[FONAY][1500]` — 60,000 `db` values, 40
        // rows of 1,500, every value a 0-9 shade.
        let strip = parse_strip(Path::new(TREE_FONT)).unwrap();
        assert_eq!(strip.len(), 60_000);
        assert_eq!(strip.len() / FONT_WIDTH, 40, "the raster's rows");
        assert_eq!(strip.iter().copied().max(), Some(9));

        // The parser refuses what it cannot read rather than skipping it: a
        // file whose lines are not `db` is not this file.
        let broken = std::env::temp_dir().join(format!("sr-cred-{}.inc", std::process::id()));
        std::fs::write(&broken, "dw 1,2\n").unwrap();
        let error = parse_strip(&broken).unwrap_err();
        assert!(error.contains("is not a `db` line"), "{error}");
        std::fs::write(&broken, "db 1,2,3\n").unwrap();
        let error = parse_strip(&broken).unwrap_err();
        assert!(error.contains("not a whole number"), "{error}");
        let _ = std::fs::remove_file(&broken);
    }

    #[test]
    fn the_images_font_and_order_are_the_trees_byte_for_byte() {
        // A8's question, asked of the release: the strip the shipped routine reads is the far
        // array at image `0x29f0` (`mov dx,029fh` at `0x1e83`), 40 rows of 1,500 up to the first
        // picture at `0x11450` — and it is the tree's `FONA.INC`, all 60,000 bytes. `fonaorder`
        // (the far pointer at DGROUP `0x92`, pointing at DGROUP `0x96`) is the tree's 47
        // characters. The routine's 32-row window is the image's too (`cmp word [bp-4],20h` at
        // `0x1e95`), so A8 stands unchanged.
        let image = image();
        let data = image_data(&image).unwrap();
        assert_eq!(FONT_AT, 0x29f0);
        assert_eq!(
            &image.image[0x1e83..0x1e86],
            &[0xba, 0x9f, 0x02],
            "mov dx,029fh"
        );
        assert_eq!(
            &image.image[0x1db0..0x1db3],
            &[0xba, 0x9f, 0x02],
            "mov dx,029fh in prt"
        );
        assert_eq!(
            &image.image[0x1e7b..0x1e7e],
            &[0xba, 0xdc, 0x05],
            "mov dx,05dch: 1,500"
        );
        assert_eq!(
            &image.image[0x1e95..0x1e99],
            &[0x83, 0x7e, 0xfc, 0x20],
            "FONAY 32"
        );
        assert_eq!(data.raster.len(), FONT_ARRAY_ROWS * FONT_WIDTH);
        assert_eq!(data.raster, parse_strip(Path::new(TREE_FONT)).unwrap());
        assert_eq!(
            FONAORDER_PTR + 4,
            DGROUP + 0x96,
            "the string follows its pointer"
        );
        assert_eq!(data.order, TREE_FONT_ORDER);
    }

    #[test]
    fn the_metrics_are_the_runs_the_strip_itself_holds() {
        // `MAIN.C:250-269`: one ink run per `fonaorder` character, in order —
        // 47 runs for 47 characters, 447 inked columns, the last ink at column
        // 557.
        let (_, part) = started();
        let mut inked_columns = 0usize;
        for &ch in &part.order {
            let width = part.fonaw[usize::from(ch)];
            assert!(width > 0, "{:?} got no run", ch as char);
            let start = usize::from(part.fonap[usize::from(ch)]);
            assert!((0..FONT_ROWS).any(|y| part.strip[y * FONT_WIDTH + start] != 0));
            inked_columns += usize::from(width);
        }
        assert_eq!(part.order.len(), 47);
        assert_eq!(inked_columns, 447, "the strip's inked columns");
        let widths: Vec<usize> = part
            .order
            .iter()
            .map(|&ch| usize::from(part.width(ch)))
            .collect();
        assert_eq!(widths.iter().copied().max(), Some(11), "the widest run");
        assert_eq!(widths.iter().copied().min(), Some(2), "the narrowest run");
        assert_eq!(
            usize::from(part.column(b'-')) + usize::from(part.width(b'-')) - 1,
            557,
            "the strip's last inked column"
        );
        assert_eq!(part.column(b'A'), 7);
        assert_eq!(part.width(b'A'), 11);
        assert_eq!(part.width(b'I'), 2, "the narrow I");
        assert_eq!(part.width(b'W'), 11);
        assert_eq!(part.width(b'-'), 10);
        assert_eq!(part.width(b'!'), 2);
        assert_eq!(part.width(b'.'), 2, "the full stop");
        assert_eq!(part.width(b'"'), 3, "the quote is the widest of the marks");
        assert_eq!(
            part.fonap[usize::from(b'-')],
            548,
            "the last of the 47 runs"
        );
        // And the routine's own blank, which no run provides.
        assert_eq!(part.column(BLANK), 1468);
        assert_eq!(part.width(BLANK), 8);
        // The runs the scan does not reach are zero, as in the source's static
        // tables: no `fonaorder` character is left out on this strip, so the
        // table entry for a character that is neither is untouched.
        assert_eq!(part.width(b'z'), 0);
        assert_eq!(part.column(b'z'), 0);
    }

    #[test]
    fn the_credit_text_is_centred_by_the_strips_own_metrics() {
        // `MAIN.C:236-243`: the line's width is the sum of `fonaw[ch]+2`, and
        // it starts at `160 - width/2`. "GRAPHICS - MARVEL" is 205 columns, so
        // the first glyph's ink starts at column 58; its pixels are the strip's
        // own, at `font[row][7..18]`, drawn at logical line 16 + row.
        let (mut rig, mut part) = started();
        let line = b"GRAPHICS - MARVEL";
        assert_eq!(part.screens[0].text[0], line);
        let width = line_width(&part, line);
        // 13 letters at 11 columns, `I` at 2, two blanks at 8, the dash at 10,
        // and 2 columns of gap per character: 171 + 34.
        assert_eq!(width, 205);
        let start = TEXT_CENTRE - width / 2;
        assert_eq!(start, 58);

        part.frame(&mut rig.ctx(PARKED_TICK));
        part.frame(&mut rig.ctx(SHOW_TICK));
        let column0 = usize::from(part.column(b'G'));
        assert_eq!(column0, 79, "`G` is the strip's seventh run");
        for row in 0..FONT_ROWS {
            for column in 0..usize::from(part.width(b'G')) {
                let expected = part.strip[row * FONT_WIDTH + column0 + column];
                let at = (TEXT_FIRST_LINE + row) * LINE_LEN + start + column;
                assert_eq!(rig.surface.pixels()[at], expected, "({column}, {row})");
            }
        }
        // The last glyph of the line, and the two-column gap after it.
        let last = start + width - 2;
        assert_eq!(
            rig.surface.pixels()[TEXT_FIRST_LINE * LINE_LEN + last - 1],
            part.strip[usize::from(part.column(b'L')) + usize::from(part.width(b'L')) - 1],
            "the last glyph's last column"
        );
        for row in 0..FONT_ROWS {
            for gap in 0..2 {
                assert_eq!(
                    rig.surface.pixels()[(TEXT_FIRST_LINE + row) * LINE_LEN + last + gap],
                    0,
                    "the gap after the line is blank"
                );
            }
        }
        // A *second* line is one `FONAY+10` further down, not over the first.
        assert_eq!(TEXT_LINE_STEP, 42);
        let second = part.screens[0].text[1].clone();
        let second_start = TEXT_CENTRE - line_width(&part, &second) / 2;
        assert!(second_start > start, "a narrower line starts further right");
        assert_eq!(
            rig.surface.pixels()[(TEXT_FIRST_LINE + TEXT_LINE_STEP) * LINE_LEN + second_start],
            part.strip[usize::from(part.column(b'M'))],
            "the second line starts 42 logical lines below the first"
        );
    }

    /// `main`'s 21 calls as the image holds them: (the call's image offset, the picture's
    /// segment, the caption's DGROUP offset, the caption's lines).
    const RELEASE_SCREENS: [(usize, u16, u16, &[&str]); 21] = [
        (
            0x193a,
            0x1145,
            0x0f2,
            &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - WILDFIRE"],
        ),
        (
            0x1955,
            0x155e,
            0x124,
            &[
                "GRAPHICS - MARVEL",
                "MUSIC - SKAVEN",
                "CODE - PSI",
                "OBJECTS - WILDFIRE",
            ],
        ),
        (
            0x1970,
            0x1977,
            0x164,
            &[
                "GRAPHICS - MARVEL",
                "MUSIC - SKAVEN",
                "CODE - WILDFIRE",
                "ANIMATION - TRUG",
            ],
        ),
        (0x198b, 0x1d90, 0x1a7, &["", "GRAPHICS - PIXEL"]),
        (
            0x19a6,
            0x21a9,
            0x1ba,
            &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x19c1,
            0x25c2,
            0x1ed,
            &["", "MUSIC - PURPLE MOTION", "CODE - TRUG"],
        ),
        (
            0x19dc,
            0x29db,
            0x211,
            &["", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x19f7,
            0x2df4,
            0x234,
            &["", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x1a12,
            0x320d,
            0x257,
            &["", "GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION"],
        ),
        (
            0x1a2d,
            0x3626,
            0x280,
            &[
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - TRUG",
                "RENDERING - TRUG",
            ],
        ),
        (
            0x1a48,
            0x3a3f,
            0x2c5,
            &[
                "SKETCH - SKAVEN",
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - PSI",
            ],
        ),
        (
            0x1a63,
            0x3e58,
            0x308,
            &[
                "SKETCH - SKAVEN",
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - PSI",
            ],
        ),
        (
            0x1a7e,
            0x4271,
            0x34b,
            &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"],
        ),
        (
            0x1a99,
            0x468a,
            0x373,
            &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"],
        ),
        (
            0x1ab4,
            0x4aa3,
            0x39b,
            &["", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x1acf,
            0x4ebc,
            0x3be,
            &[
                "GRAPHICS - PIXEL",
                "MUSIC - PURPLE MOTION",
                "CODE - TRUG",
                "RENDERING - TRUG",
            ],
        ),
        (
            0x1aea,
            0x52d5,
            0x403,
            &["", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x1b05,
            0x56ee,
            0x426,
            &["GRAPHICS - MARVEL", "MUSIC - PURPLE MOTION", "CODE - PSI"],
        ),
        (
            0x1b20,
            0x5b07,
            0x45a,
            &["MUSIC - SKAVEN", "CODE - PSI", "WORLD - TRUG"],
        ),
        (
            0x1b3b,
            0x5f20,
            0x482,
            &["GRAPHICS - PIXEL", "MUSIC - SKAVEN"],
        ),
        (
            0x1b56,
            0x6339,
            0x4a3,
            &["GRAPHICS - PIXEL", "MUSIC - SKAVEN", "CODE - WILDFIRE"],
        ),
    ];

    #[test]
    fn the_screens_are_mains_twenty_one_calls_read_from_the_image() {
        // `main` (`0x190c-0x1b68`) is 21 `if(!dis_exit()) screenin(pic, text)` calls, and each
        // call's bytes say where its picture and caption are: the picture at image
        // `segment * 16`, the 21 segments 0x419 paragraphs (one 16,784-byte blob) apart from
        // `0x1145`, and the caption at `DGROUP + text`, DGROUP being `mov dx,6752h` at image 0.
        let image = image();
        assert_eq!(&image.image[..3], &[0xba, 0x52, 0x67], "mov dx,6752h");
        assert_eq!(DGROUP, 0x67520);
        let data = image_data(&image).unwrap();
        assert_eq!(data.calls.len(), SCREEN_COUNT);
        for (number, (call, (at, segment, text, lines))) in
            data.calls.iter().zip(RELEASE_SCREENS).enumerate()
        {
            let screen = number + 1;
            assert_eq!(call.at, at, "screen {screen}'s call");
            assert_eq!(call.segment, segment, "screen {screen}'s picture");
            assert_eq!(call.text, text, "screen {screen}'s caption");
            assert_eq!(
                usize::from(segment),
                0x1145 + number * 0x419,
                "screen {screen}"
            );
            assert_eq!(PICTURE_BYTES, 0x419 * 16);
            let lines: Vec<&[u8]> = lines.iter().map(|line| line.as_bytes()).collect();
            assert_eq!(
                data.captions[number], lines,
                "screen {screen}'s caption lines"
            );
            // Each picture is the tree's own blob layout: `FC FC`, 160x100, 256 colours, the
            // pixels at paragraph 49 (offset 784).
            let blob = &data.pictures[number];
            assert_eq!(
                &blob[..10],
                &[0xfc, 0xfc, 0xa0, 0x00, 0x64, 0x00, 0x00, 0x01, 0x31, 0x00],
                "screen {screen}'s header"
            );
        }
        // The calls step by 27 bytes: each is the `dis_exit` test (`lcall 0291:005a; or ax,ax;
        // jne`, 12 bytes) and the 15-byte call with its `add sp,8`.
        assert!(data
            .calls
            .windows(2)
            .all(|pair| pair[1].at - pair[0].at == 0x1b));
        // And the part's screens are those: picture, palette and caption.
        let (_, part) = started();
        for (number, screen) in part.screens.iter().enumerate() {
            let decoded = load(&data.pictures[number], Format::Origin).unwrap();
            assert_eq!(screen.picture, decoded.pixels(), "screen {}", number + 1);
            assert_eq!(screen.text, data.captions[number], "screen {}", number + 1);
        }
    }

    #[test]
    fn an_image_without_this_builds_calls_is_refused() {
        // The scan refuses rather than guesses: a call that is not there, or one too many, is a
        // different `main`.
        let mut image = image();
        image.image[0x1b56] = 0x90;
        let error = image_data(&image).err().unwrap();
        assert!(error.contains("20 screenin calls"), "{error}");
        let mut image = self::image();
        image.relocations.retain(|&at| at != 0x193f);
        let error = image_data(&image).err().unwrap();
        assert!(error.contains("not relocated"), "{error}");
    }

    #[test]
    fn the_release_changes_nine_of_the_trees_screens() {
        // What the release changed, measured against the tree's own data rather than against
        // frames: each screen's picture, the palette entries it shows (repaired) and its caption,
        // the image's against `TREE_SCREENS`. Nine screens differ — A10's nine, which the
        // record's hold samples found — and the other twelve are the tree's pictures, on the
        // entries they show, with the tree's captions.
        //
        // `(screen, picture pixels that differ, shown palette entries that differ, caption
        // differs)`.
        let (_, part) = started();
        let mut changed = Vec::new();
        let mut unshown = Vec::new();
        for (number, (file, lines)) in TREE_SCREENS.iter().enumerate() {
            let tree = sr_assets::load_path(Path::new(&format!("{TREE_PICTURES}/{file}"))).unwrap();
            let mut palette = tree.palette;
            repair_palette(&mut palette);
            let screen = &part.screens[number];
            let pixels = screen
                .picture
                .iter()
                .zip(tree.pixels())
                .filter(|(a, b)| a != b)
                .count();
            // The entries the screen shows: the caption's ramp (0-9) and every `v + 16` either
            // picture uses. An entry no pixel indexes is not on the screen.
            let mut shown: std::collections::BTreeSet<u8> = (0..RAMP_ENTRIES).collect();
            shown.extend(
                screen
                    .picture
                    .iter()
                    .chain(tree.pixels())
                    .map(|v| v.wrapping_add(16)),
            );
            let differs = |i: &u8| screen.palette.entry(*i) != palette.entry(*i);
            let entries = shown.iter().filter(|i| differs(i)).count();
            unshown.push((0..=255u8).filter(differs).count() - entries);
            let tree_lines: Vec<&[u8]> = lines.iter().map(|line| line.as_bytes()).collect();
            let caption = screen.text != tree_lines;
            if pixels + entries > 0 || caption {
                changed.push((number + 1, pixels, entries, caption));
            }
        }
        assert_eq!(
            changed,
            [
                (2, 15_996, 223, true),
                (6, 15_997, 44, false),
                (10, 15_452, 231, false),
                (11, 0, 0, true),
                (12, 0, 0, true),
                (17, 15_992, 156, false),
                (19, 15_998, 234, true),
                (20, 15_990, 239, true),
                (21, 15_882, 32, true),
            ]
        );
        // The twelve unchanged screens' blobs differ from the LBMs only in entries no pixel of
        // theirs indexes — the blob's palette is not the LBM's past the colours the picture uses
        // on seven of them.
        let unchanged: Vec<(usize, usize)> = (1..=SCREEN_COUNT)
            .filter(|screen| !changed.iter().any(|(changed, ..)| changed == screen))
            .map(|screen| (screen, unshown[screen - 1]))
            .collect();
        assert_eq!(
            unchanged,
            [
                (1, 129),
                (3, 83),
                (4, 74),
                (5, 0),
                (7, 128),
                (8, 58),
                (9, 0),
                (13, 27),
                (14, 0),
                (15, 81),
                (16, 0),
                (18, 0),
            ]
        );
    }

    #[test]
    fn the_sixth_screen_draws_its_own_picture() {
        // Five screens in, on the tick after screen 6's parked one: the tick its palette and its
        // picture land on. The picture is the image's blob at segment `0x25c2`, drawn at `+16`.
        let (mut rig, mut part) = started();
        let sixth = part.screens[5].picture.clone();
        assert_eq!(sixth.len(), PICTURE_WIDTH * PICTURE_HEIGHT);
        let ticks = 5 * part.screen_ticks() + SHOW_TICK + 1;
        run_ticks(&mut rig, &mut part, ticks);
        for (y, row) in sixth.chunks_exact(PICTURE_WIDTH).enumerate() {
            for (x, value) in row.iter().enumerate() {
                let at = (PICTURE_FIRST_LINE + PICTURE_LINE_STEP * y) * LINE_LEN
                    + PICTURE_FIRST_COLUMN
                    + x;
                assert_eq!(
                    rig.surface.pixels()[at],
                    value.wrapping_add(PICTURE_COLOUR_OFFSET),
                    "({x}, {y})"
                );
            }
        }
    }

    #[test]
    fn the_picture_is_laid_in_at_the_coordinates_the_source_gives() {
        // `:174`'s `tw_putpixel(400+x, 400+y*2, pic[y][x]+16)` in the port's
        // flat units: picture pixel (x, y) is flat `(400+2y)*320 + 400 + x`.
        // The pen's byte address is `y*80 + x/4`, so the *displayed* row is
        // flat line `401 + 2y` and column `80 + x` — the picture's column 0 is
        // displayed at column 80, which the frame at 1440 shows.
        let (mut rig, mut part) = started();
        let picture = part.screens[0].picture.clone();
        run_ticks(&mut rig, &mut part, SHOW_TICK + 1);
        let mut seen = 0usize;
        for (y, row) in picture.chunks_exact(PICTURE_WIDTH).enumerate() {
            for (x, value) in row.iter().enumerate() {
                let flat = (PICTURE_FIRST_LINE + PICTURE_LINE_STEP * y) * LINE_LEN
                    + PICTURE_FIRST_COLUMN
                    + x;
                assert_eq!(
                    rig.surface.pixels()[flat],
                    value.wrapping_add(PICTURE_COLOUR_OFFSET),
                    "({x}, {y})"
                );
                if *value != 0 {
                    seen += 1;
                }
            }
        }
        assert!(seen > 1_000, "the picture's first screen has {seen} non-zero pixels");
        // The display reads the odd logical line: the picture's row 0 starts at
        // display row 0, column 80.
        assert_eq!(
            rig.surface.pixels()[HOLD_ORIGIN as usize + 80],
            picture[0].wrapping_add(PICTURE_COLOUR_OFFSET),
            "the picture's first pixel is the display's row 0, column 80"
        );
    }

    #[test]
    fn the_palette_repair_moves_entries_zero_to_239_up_by_sixteen() {
        // `MAIN.C:273-293`: 720 bytes from entry 0 to entry 16, so entries 0-15
        // keep their values *and* appear again at 16-31.
        let mut palette = Palette::new();
        for index in 0u8..=255 {
            palette.set_entry(index, [index % 64, 1, 2]);
        }
        repair_palette(&mut palette);
        assert_eq!(palette.entry(16), [0, 1, 2], "entry 0's colour, at 16");
        assert_eq!(palette.entry(255), [239 % 64, 1, 2], "entry 239's, at 255");
        assert_eq!(palette.entry(10), [10, 1, 2], "10 is below the ramp's reach");
        assert_eq!(palette.entry(15), [15, 1, 2], "and so is 15");
        assert_eq!(palette.entry(16 + 15), [15, 1, 2], "entry 15 appears at 31");
    }

    #[test]
    fn the_ten_entry_grey_ramp_lands_on_entries_zero_to_nine() {
        // `MAIN.C:294-317`: `pic[a*3+k+16] = 7*a` for `a` 0-9 — and the palette
        // starts at offset 16 of the picture, so those are entries 0-9, the
        // ones the *caption* is drawn with (the strip's own values are 0-9).
        // The ramp is what makes the text grey; the picture's `+16` is what
        // keeps the picture's own colours.
        let mut palette = Palette::new();
        for index in 0u8..=255 {
            palette.set_entry(index, [63, 63, 63]);
        }
        repair_palette(&mut palette);
        for a in 0..10u8 {
            assert_eq!(palette.entry(a), [7 * a, 7 * a, 7 * a], "entry {a}");
        }
        assert_eq!(palette.entry(9), [63, 63, 63], "the ramp's last is white");
        assert_eq!(palette.entry(10), [63, 63, 63], "entry 10 is not the ramp's");
        // The caption's colours are therefore the ramp's, at the strip's own
        // values: the text's whitest pixels are 9, which is 63.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, SHOW_TICK + 1);
        // The caption's pixels are the strip's own values, so the screen's
        // entries they index are 0-9 — the ramp's — and their shades are the
        // strip's, widest included.
        let mut shades = std::collections::BTreeSet::new();
        for row in 0..TEXT_LINE_STEP {
            for column in 0..320 {
                let at = (TEXT_FIRST_LINE + row) * LINE_LEN + column;
                shades.insert(rig.surface.pixels()[at]);
            }
        }
        assert!(shades.iter().all(|shade| *shade < RAMP_ENTRIES), "{shades:?}");
        assert!(shades.contains(&9), "the caption's brightest value is the strip's 9");
        assert!(shades.contains(&0), "and its background is the strip's 0");
    }

    #[test]
    fn the_hold_is_two_hundred_frames_and_it_does_not_move() {
        // `MAIN.C:194`: `for(a=0;a<200 && !dis_exit();a++) dis_waitb();`. The
        // hold is the ticks between the open loop's last step and the close
        // loop's first, and neither the screen nor the display moves across it.
        assert_eq!(HOLD_FRAMES, 200);
        let (mut rig, mut part) = started();
        // Tick 2 is the open loop's first step and tick `SHOW_TICK + OPEN_STEPS`
        // = 103 its last, which is the tick that writes the hold's display
        // state; the hold's own 200 frames are 104..303.
        let open_last = SHOW_TICK + OPEN_STEPS;
        assert_eq!(open_last, 103);
        let hold_first = open_last + 1;
        run_ticks(&mut rig, &mut part, hold_first);
        let screen = rig.surface.pixels().to_vec();
        let view = rig.view.clone();
        let palette = rig.palette;
        for tick in hold_first..hold_first + HOLD_FRAMES {
            assert_eq!(part.frame(&mut rig.ctx(tick)), PartStatus::Continuing);
            assert_eq!(rig.surface.pixels(), &screen[..], "tick {tick} moved the screen");
            assert_eq!(rig.view, view, "tick {tick} moved the display");
            assert_eq!(rig.palette, palette, "tick {tick} moved the DAC");
        }
        // And the tick the hold ends on is the close loop's first step.
        assert_eq!(part.tick, hold_first + HOLD_FRAMES);
    }

    #[test]
    fn the_part_runs_twenty_one_screens_of_the_sources_own_length() {
        // 21 screens, 363 ticks each, then `MAIN.C:158-159`'s `}`.
        assert_eq!(OPEN_STEPS, 102, "the open loop's own count");
        assert_eq!(CLOSE_STEPS, 59, "the close loop's own count");
        let (mut rig, mut part) = started();
        assert_eq!(
            part.screens.len(),
            SCREEN_COUNT,
            "`MAIN.C:41-156`'s calls, and the image's"
        );
        assert_eq!(SCREEN_COUNT, 21);
        assert_eq!(part.screen_ticks(), 363);
        assert_eq!(part.total_ticks(), 7_623);
        let last = part.total_ticks() - 1;
        run_ticks(&mut rig, &mut part, last);
        // The last tick draws the last screen's last frame *and* ends `main()`:
        // the shell composes that tick's frame and hands the next one to
        // ENDSCRL, exactly as the original's loader does.
        assert_eq!(part.frame(&mut rig.ctx(last)), PartStatus::Finished);
        assert_eq!(part.screen, SCREEN_COUNT);
    }

    #[test]
    fn the_wipes_ramps_are_the_sources_own_sequences() {
        // `MAIN.C:176`: `y = y*12L/13` from 200*128, 102 steps, the last 1 —
        // and `HOLD_ORIGIN` is exactly what that last step's `yy` produces.
        let open = open_ramp();
        assert_eq!(open.len(), OPEN_STEPS as usize);
        assert_eq!(open[0], 200 * 128);
        assert_eq!(open.last(), Some(&1));
        let last = u32::from(*open.last().unwrap());
        let yy = 320 - last / 80;
        assert_eq!(yy, 320);
        assert_eq!(
            (160 * 200 + yy / 4) * 4,
            HOLD_ORIGIN,
            "tw_setstart(160*200 + yy/4): 80 bytes of four-pixel addresses"
        );
        assert_eq!(last / 128 + 200, u32::from(HOLD_LINE_COMPARE));
        assert_eq!(((yy & 3) * 2) as u8, HOLD_PAN);

        // `MAIN.C:196`: `y = y+v, v += 15` until 128*200, 59 steps.
        let close = close_ramp();
        assert_eq!(close.len(), CLOSE_STEPS as usize);
        assert_eq!(close[0], (0, 0));
        assert_eq!(close.last(), Some(&(24795, 870)));
        let (last_y, last_v) = *close.last().unwrap();
        assert!(
            last_y + last_v >= 128 * 200,
            "the loop's own next y must end it: {last_y} + {last_v}"
        );
    }

    #[test]
    fn the_parked_split_is_below_the_display_and_the_holds_is_half_way() {
        // `tw_setsplit(400)` (`MAIN.C:165`) is line compare 400; the display is
        // 200 rows, so it is inert. `tw_setsplit(200)` (`:179`) puts the lower
        // view's first row at 100. Both come from the register's own convention
        // plus the double scan, which is `(line_compare + 1) / 2`.
        assert_eq!(split_scanline(400), 200);
        assert_eq!(split_scanline(200), 100);
        assert_eq!(split_scanline(0), 0);

        let (mut rig, mut part) = started();
        part.frame(&mut rig.ctx(PARKED_TICK));
        let parked = rig.view.clone();
        assert_eq!(parked.start_addr, PARKED_ORIGIN);
        assert_eq!(parked.y_line_offset, Some(DISPLAY_LINE_STRIDE));
        assert!(!parked.double_scan);
        assert_eq!(parked.split.as_ref().unwrap().scanline, VIEW_HEIGHT);
        // Inert: every displayed row above it is the enclosing view's.
        for row in [0u16, 99, 199] {
            assert_eq!(parked.effective(row).start_addr, PARKED_ORIGIN, "row {row}");
        }

        run_ticks(&mut rig, &mut part, SHOW_TICK + OPEN_STEPS);
        let hold = rig.view.clone();
        assert_eq!(hold.start_addr, HOLD_ORIGIN);
        assert_eq!(hold.start_addr - PARKED_ORIGIN, LINE_LEN as u32, "one line's scroll");
        assert_eq!(hold.start_addr / LINE_LEN as u32, 401, "an odd logical line");
        assert_eq!(hold.pan, HOLD_PAN);
        assert_eq!(hold.split.as_ref().unwrap().scanline, 100);
        // The picture's own line above the split, the caption's page below it.
        assert_eq!(hold.effective(0).start_addr, HOLD_ORIGIN);
        assert_eq!(hold.effective(99).start_addr, HOLD_ORIGIN);
        let lower = hold.effective(100);
        assert_eq!(lower.start_addr, 0, "the split resets the address counter");
        assert_eq!(lower.pan, 0, "and clears panning");
        assert_eq!(
            lower.stride(SURFACE_WIDTH as u16),
            DISPLAY_LINE_STRIDE,
            "the line offset register is unchanged below the split"
        );
        assert_eq!(hold.source_index(&rig.surface, 0, 100), 0);
        assert_eq!(
            hold.source_index(&rig.surface, 319, 199),
            (199 - 100) * 640 + 319,
            "scanline 199 is the lower view's own row 99"
        );
    }

    #[test]
    fn the_holds_display_state_reproduces_the_references_own_frames() {
        // The strongest statement available about the geometry, the palette
        // repair, the font, the image's data and the display state at once: the
        // port's screens 1, 6, 12, 18 and 21 are `reference/parts/cred`'s five
        // frames, 1440, 3240, 5440, 7640 and 8740, byte for byte. Three of them
        // (6, 12, 21) are screens the release changed from the tree (A10, retired).
        //
        // It is also what fixes `HOLD_ORIGIN`: at the *parked* origin the
        // display reads the even logical lines, which hold no picture.
        let (mut rig, mut part) = started();
        let hold_first = SHOW_TICK + OPEN_STEPS + 1;

        run_ticks(&mut rig, &mut part, hold_first);
        let first = rig.rendered_rgb();
        let at_1440 = reference("cred", 1440, "rgb");
        let differing = differing_bytes(&first, &at_1440);
        assert_eq!(differing, 0, "screen 1 differs from the reference in {differing} bytes");

        // The parked state is not the hold's: the same frame drawn from the
        // parked origin is a different picture.
        let hold_view = rig.view.clone();
        Cred::set_display(&mut rig.ctx(hold_first), Display::PARKED);
        let parked = rig.rendered_rgb();
        let parked_differing = differing_bytes(&parked, &at_1440);
        assert_eq!(
            parked_differing, 29_523,
            "the parked display reads the even logical lines, which hold no picture; it differs \
             from the reference's frame in {parked_differing} bytes"
        );
        rig.view = hold_view;

        // The other four, each the same number of screens further on.
        let mut at = 1;
        for (screen, keyframe) in [(6, 3240), (12, 5440), (18, 7640), (21, 8740)] {
            let ticks = (screen - at) * part.screen_ticks();
            run_ticks(&mut rig, &mut part, ticks);
            at = screen;
            let frame = rig.rendered_rgb();
            let expected = reference("cred", keyframe, "rgb");
            let differing = differing_bytes(&frame, &expected);
            assert_eq!(
                differing, 0,
                "screen {screen} differs from the reference's {keyframe} in {differing} bytes"
            );
        }
    }

    #[test]
    fn the_split_register_is_one_more_than_the_line_compare_odd_or_even() {
        // `tw_setsplit`'s register names the last line *above* the split, so
        // the first row below it is that line plus one; the mode is
        // double-scanned and the port renders it single-scanned, so a displayed
        // row is two scanlines: `(line_compare + 1) / 2`. Even and odd values
        // are both arithmetic here — a register of 205 lands on row 103, not
        // 102 — and both are measured against a capture, not assumed:
        //
        // - **even, 200** — the hold: `reference/parts/cred`'s 1440 and 7640
        //   are reproduced byte for byte at row 100
        //   (`the_holds_display_state_reproduces_the_references_own_frames`);
        // - **odd, 205** — the close wipe's step 10: the record's own
        //   `out/b5-s5-cred-map/frame_001540.rgb` is reproduced byte for byte at
        //   row 103 (`the_records_transition_frames_are_the_states_this_part_writes`).
        assert_eq!(split_scanline(200), 100);
        assert_eq!(split_scanline(205), 103);
        assert_eq!(split_scanline(204), 102);
        assert_eq!(split_scanline(206), 103);
        assert_eq!(split_scanline(201), 101);
        // And the ramp's own values: `tw_setsplit(400)` is the first row below
        // a 200-row display (inert), and the close's last step is 393.
        assert_eq!(split_scanline(400), 200);
        assert_eq!(split_scanline(393), 197);
        assert_eq!(split_scanline(384), 192);
        assert_eq!(split_scanline(0), 0);
    }

    #[test]
    fn the_pan_register_is_the_low_two_bits_of_yy_doubled() {
        // `MAIN.C:183-191`: the register is written with `(yy & 3) * 2`. That is
        // the *register*, and the pixels it moves the display by are half of
        // it: the emulator's 256-colour arm is `pel_panning = (val & 0x7) / 2`
        // (`vga_attr.cpp:225-227`), in the unit the display start itself is
        // counted in (`vga_draw.cpp:1648-1651`). So a step's pan is `yy & 3`
        // pixels, and the register is `2 * that`.
        //
        // Both numbers are pinned against the record below: the eleven
        // transition frames whose `yy` is not a multiple of 4 are reproduced
        // only by the halved reading, and the register-value reading misses
        // them by thousands of bytes.
        for (yy, register) in [(0u16, 0u8), (1, 2), (2, 4), (3, 6), (4, 0), (5, 2), (6, 4)] {
            assert_eq!(pan(yy) * 2, register, "yy {yy}: AC 0x13");
            assert_eq!(pan(yy), register / 2, "yy {yy}: the pixel shift");
        }
        // The states the record's transition frames are, whose pan is not 0.
        for (yy, register, pixels) in [
            (25u16, 2u8, 1u8),   // screen 1, open step 1
            (69, 2, 1),          // screen 1, open step 3
            (321, 2, 1),         // screen 5, close step 45
            (328, 0, 0),         // screen 1, close step 10 (1540)
            (531, 6, 3),         // screen 14, close step 48 (6340)
            (629, 2, 1),         // the close loop's last step
            (320, 0, 0),         // the hold
        ] {
            assert_eq!(pan(yy), pixels, "yy {yy}");
            assert_eq!(pan(yy) * 2, register, "yy {yy}");
        }
    }

    #[test]
    fn the_two_ramps_write_the_three_registers_the_source_writes() {
        // `MAIN.C:179-191` (open) and `:199-211` (close), step by step, with
        // the values the source's own integer arithmetic gives — not a curve
        // that "matches" the loop. (step, `y`, line compare, display start in
        // pixels, AC 0x13's register value).
        let open = open_ramp();
        for (step, y, line_compare, origin, register) in [
            (0usize, 25_600u16, 400u16, 128_000u32, 0u8),
            (1, 23_630, 384, 128_024, 2),
            (2, 21_812, 370, 128_048, 0),
            (3, 20_134, 357, 128_068, 2),
            (4, 18_585, 345, 128_088, 0),
            (101, 1, 200, 128_320, 0),
        ] {
            assert_eq!(open[step], y, "open step {step}: the loop's own `y`");
            let display = Display::opening(open[step]);
            assert_eq!(display.line_compare, line_compare, "open step {step}");
            assert_eq!(display.origin, origin, "open step {step}");
            assert_eq!(display.pan * 2, register, "open step {step}: AC 0x13");
        }

        let close = close_ramp();
        for (step, y, line_compare, origin, register) in [
            (0usize, 0u16, 200u16, 128_320u32, 0u8),
            (1, 0, 200, 128_320, 0),
            (2, 15, 200, 128_320, 0),
            (4, 90, 200, 128_320, 2),
            (10, 675, 205, 128_328, 0),
            (11, 825, 206, 128_328, 4),
            (58, 24_795, 393, 128_628, 2),
        ] {
            assert_eq!(close[step].0, y, "close step {step}: the loop's own `y`");
            let display = Display::closing(close[step].0);
            assert_eq!(display.line_compare, line_compare, "close step {step}");
            assert_eq!(display.origin, origin, "close step {step}");
            assert_eq!(display.pan * 2, register, "close step {step}: AC 0x13");
        }

        // The two ramps meet where the source says they do: the open loop's
        // last step writes the parked state again — `y = 200*128` is `yy = 0`
        // and a line compare of 400 — and the close loop's first three steps
        // write the hold's state exactly, so the four states a screen passes
        // through endpoint-to-endpoint are in the loops themselves.
        assert_eq!(Display::opening(open[0]), Display::PARKED);
        let hold = hold_state();
        assert_eq!(Display::opening(*open.last().unwrap()), hold);
        for (step, (y, _)) in close.iter().enumerate().take(4) {
            assert_eq!(Display::closing(*y), hold, "close step {step}");
        }
    }

    #[test]
    fn the_three_registers_move_once_a_tick_through_both_wipes() {
        // One tick is one `dis_waitb`, so the tick each step is *displayed* on
        // is the step's own: tick 2 is the open loop's first (`MAIN.C:176`) and
        // tick `SHOW_TICK + OPEN_STEPS` = 103 its last, the hold's 200 frames
        // are 104-303 with nothing written (`:194`), and the close loop's 59
        // steps are 304-362 (`:196`). The screen's last tick is 362 and the
        // next screen's tick 0 re-parks the display.
        let (mut rig, mut part) = started();
        let open = part.open.clone();
        let close = part.close.clone();
        let open_first = SHOW_TICK + 1;
        let hold_first = open_first + open.len() as u32;
        let close_first = hold_first + HOLD_FRAMES;
        assert_eq!((open_first, hold_first, close_first), (2, 104, 304));
        assert_eq!(part.screen_ticks(), close_first + close.len() as u32);

        // The literal states at named ticks: the display start, the split row
        // and the pan in pixels, hand-computed from the two loops. The ticks
        // either side of every boundary are in here, because a step landing one
        // tick out is exactly what this checks.
        let named: std::collections::HashMap<u32, (u32, u16, u8)> = [
            (0, (128_000, 200, 0)),   // parked (`:165-167`), inert split
            (1, (128_000, 200, 0)),   // the palette and the picture land here
            (2, (128_000, 200, 0)),   // open step 0: `y = 200*128`, `yy = 0`
            (3, (128_024, 192, 1)),   // open step 1: `y` 23,630, `yy` 25
            (4, (128_048, 185, 0)),   // open step 2: `y` 21,812, `yy` 48
            (5, (128_068, 179, 1)),   // open step 3: `y` 20,134, `yy` 69
            (103, (128_320, 100, 0)), // the last open step: the hold's state
            (104, (128_320, 100, 0)), // the hold's first frame
            (303, (128_320, 100, 0)), // and its last
            (304, (128_320, 100, 0)), // close step 0
            (307, (128_320, 100, 0)), // close step 3: still `yy` 320
            (308, (128_320, 100, 1)), // close step 4: `yy` 321
            (314, (128_328, 103, 0)), // close step 10: `y` 675, `yy` 328
            (315, (128_328, 103, 2)), // close step 11: `yy` 330
            (362, (128_628, 197, 1)), // close step 58, the screen's last tick
        ]
        .into_iter()
        .collect();
        for tick in 0..part.screen_ticks() {
            assert_eq!(part.frame(&mut rig.ctx(tick)), PartStatus::Continuing, "tick {tick}");
            let expected = match tick {
                PARKED_TICK | SHOW_TICK => Display::PARKED,
                t if t < hold_first => Display::opening(open[(t - open_first) as usize]),
                t if t < close_first => hold_state(),
                t => Display::closing(close[(t - close_first) as usize].0),
            };
            let split = rig.view.split.as_ref().unwrap();
            assert_eq!(rig.view.start_addr, expected.origin, "tick {tick}: the start");
            assert_eq!(split.scanline, split_scanline(expected.line_compare), "tick {tick}");
            assert_eq!(rig.view.pan, expected.pan, "tick {tick}: the pan");
            if let Some((origin, row, pan)) = named.get(&tick) {
                assert_eq!(
                    (rig.view.start_addr, split.scanline, rig.view.pan),
                    (*origin, *row, *pan),
                    "tick {tick}"
                );
            }
        }

        // The split's own two conventions at a wipe step with a pan: above the
        // split the display reads the step's start and pan, below it the
        // hardware's address counter is 0 and the pan is cleared.
        let (mut rig, mut part) = started();
        // `run_ticks` counts frames, so 309 of them run ticks 0 to 308.
        run_ticks(&mut rig, &mut part, 309);
        let split = rig.view.split.as_ref().unwrap();
        assert_eq!(split.scanline, 100);
        assert_eq!(rig.view.effective(0).start_addr, 128_320);
        assert_eq!(rig.view.effective(0).pan, 1);
        assert_eq!(rig.view.effective(100).start_addr, 0);
        assert_eq!(rig.view.effective(100).pan, 0);
        assert_eq!(
            rig.view.source_index(&rig.surface, 0, 0),
            128_321,
            "the first displayed pixel is one past the start: the pan"
        );
        assert_eq!(rig.view.source_index(&rig.surface, 0, 100), 0, "below the split");
    }

    /// The hold's state as the tests name it, from the constants.
    fn hold_state() -> Display {
        Display {
            origin: HOLD_ORIGIN,
            line_compare: HOLD_LINE_COMPARE,
            pan: HOLD_PAN,
        }
    }

    // ------------------------------------------- the record's own frames --
    //
    // `out/b5-s5-cred-map` is a **scratch capture**: a sweep of every 100th
    // frame from 1240 to 8840 — 77 frames, the 78th index the command asked for
    // (8940) refused because the screen there is ENDSCRL's — taken by the
    // `cargo xtask reference capture` command in its `.log`, RGB only. It lives
    // under the git-ignored `out/`,
    // so the two tests below are `#[ignore]`d — they are a *record* of what
    // the release's own frames show, not an input the suite leans on, and a
    // fresh clone has no such directory. What they are evidence *of* is the one
    // thing this part cannot otherwise measure: **the wipes**, which no
    // keyframe covers (the reference's five CRED indices are all *held*
    // screens). The run cannot be repeated index for index — a session's
    // animation phase is not reproducible (D17) — so these frames are read as
    // they are rather than re-captured.

    /// The record's sweep of this part, by index, or `None` without the
    /// scratch directory.
    fn record_frames() -> Option<Vec<(u32, Vec<u8>)>> {
        let dir = Path::new("../../out/b5-s5-cred-map");
        if !dir.is_dir() {
            eprintln!("{} is not here: nothing to read", dir.display());
            return None;
        }
        let mut frames: Vec<(u32, Vec<u8>)> = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            let Some(index) = name
                .strip_prefix("frame_")
                .and_then(|rest| rest.strip_suffix(".rgb"))
                .and_then(|digits| digits.parse().ok())
            else {
                continue;
            };
            frames.push((index, std::fs::read(&path).unwrap()));
        }
        frames.sort();
        assert_eq!(frames.len(), 77, "the sweep is 77 frames");
        Some(frames)
    }

    /// The part on the tick screen `screen`'s palette, caption and picture land
    /// on — the content every state of that screen is drawn over — and the rig
    /// it was drawn through.
    fn at_screen(screen: usize) -> (Rig, Cred) {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT);
        let mut part = Cred::new();
        part.init(&mut rig.ctx(0)).unwrap();
        let target = screen as u32 * part.screen_ticks() + SHOW_TICK;
        // `frame` advances the part, so the tick's own call is the last one.
        while (part.screen as u32 * part.screen_ticks() + part.tick) <= target {
            part.frame(&mut rig.ctx(0));
        }
        (rig, part)
    }

    /// The frame the part shows at `display` over that screen's surface.
    fn render_at(rig: &mut Rig, display: Display) -> Vec<u8> {
        Cred::set_display(&mut rig.ctx(0), display);
        rig.rendered_rgb()
    }

    /// How many bytes of `frame` differ from `expected` over displayed rows
    /// `first..last`.
    fn differing_in(frame: &[u8], expected: &[u8], first: usize, last: usize) -> usize {
        assert_eq!(frame.len(), expected.len());
        assert_eq!(frame.len(), 320 * 200 * 3);
        (first * 960..last * 960)
            .filter(|at| frame[*at] != expected[*at])
            .count()
    }

    /// Where the port's block-5 chain starts this part: reference index 1212 is CRED's tick 0
    /// (`docs/fidelity/cred.md` §7, `tests/cred.rs`).
    const PORT_TICK_ZERO: u32 = 1212;

    /// The tick of a screen a wipe state is displayed on: open step `k` on tick `2 + k`, close
    /// step `k` on tick `304 + k` (`the_three_registers_move_once_a_tick_through_both_wipes`).
    fn state_tick(kind: &str, step: usize) -> u32 {
        let base = if kind == "open" {
            SHOW_TICK + 1
        } else {
            SHOW_TICK + 1 + OPEN_STEPS + HOLD_FRAMES
        };
        base + step as u32
    }

    /// One of the record's wipe frames: `(index, screen, loop, step, the doubled pan reading's
    /// bytes, the lag)`.
    type Transition = (u32, usize, &'static str, usize, Option<usize>, u32);

    #[test]
    #[ignore = "reads the scratch capture out/b5-s5-cred-map; a record, not an input"]
    fn the_records_transition_frames_are_the_states_this_part_writes() {
        // **The wipes, against the record.** Every frame here is a *moving*
        // frame — a state between two of the part's holds — and each one is
        // exactly one state of one screen: `(index, screen (1-based), loop,
        // step)`. The port writes that state at one tick of that screen, and
        // the frame it shows there is the record's, **byte for byte over the
        // whole 320x200 display**. With the screens read from the image, all
        // 28 of the sweep's wipe frames are answered — the twelve on the nine
        // screens the release changed as well as the sixteen on the others.
        //
        // It is the only check of the iris at all: `reference/parts/cred`'s
        // five indices are held screens, and `reference-indices.md` §20 records
        // that the reference "says nothing about the 590 frames a screen spends
        // opening and closing".
        let Some(frames) = record_frames() else {
            panic!(
                "this ignored test's input is the scratch capture `out/b5-s5-cred-map`, \
                 which is not here: there is nothing to check. Produce it with the sweep \
                 `reference-indices.md` §20 records (`cargo xtask reference capture`), or run \
                 the suite without `--ignored` — a missing capture must fail loudly, not \
                 pass silently"
            )
        };
        let byte_for = |index: u32| frames.iter().find(|(at, _)| *at == index).unwrap().1.clone();

        // The fifth column is the bytes by which the *doubled* pan reading — the
        // register value taken as pixels — differs over the whole display:
        // `None` where the step's pan is 0 (so both readings are the same
        // frame), `Some(0)` for 6340, the one frame whose visible sliver is
        // invariant under the shift, and the measured difference otherwise.
        //
        // The last column is the **lag**: the record's index minus the index the
        // port's block-5 chain shows that state at (`PORT_TICK_ZERO + (screen-1)*363 +
        // tick`). It is the chain offset ENDSCRL inherits (A11), measured screen by screen.
        let transitions: [Transition; 28] = [
            (1240, 1, "open", 12, Some(5_375), 14),
            (1540, 1, "close", 10, None, 14),
            (1640, 2, "open", 45, None, 18),
            (1940, 2, "close", 43, Some(7_415), 18),
            (2340, 4, "open", 13, None, 24),
            (2640, 4, "close", 11, Some(17_130), 24),
            (2740, 5, "open", 46, Some(6_446), 28),
            (3040, 5, "close", 45, Some(1_534), 27),
            (3440, 7, "open", 15, None, 33),
            (3740, 7, "close", 13, Some(46_722), 33),
            (3840, 8, "open", 48, Some(32_709), 37),
            (4140, 8, "close", 47, Some(7_804), 36),
            (4540, 10, "open", 16, None, 43),
            (4840, 10, "close", 14, Some(23_609), 43),
            (4940, 11, "open", 49, Some(16_196), 47),
            (5240, 11, "close", 47, Some(1_304), 47),
            (5640, 13, "open", 16, None, 54),
            (5940, 13, "close", 14, Some(3_554), 54),
            (6040, 14, "open", 50, Some(14_217), 57),
            (6340, 14, "close", 48, Some(0), 57),
            (6740, 16, "open", 17, Some(16_247), 64),
            (7040, 16, "close", 15, Some(19_301), 64),
            (7140, 17, "open", 51, Some(9_154), 67),
            (7440, 17, "close", 49, None, 67),
            (7840, 19, "open", 18, Some(7_432), 74),
            (8140, 19, "close", 16, Some(10_305), 74),
            (8240, 20, "open", 52, None, 77),
            (8540, 20, "close", 50, Some(167), 77),
        ];
        let mut ruled_out = 0;
        let mut with_pan = 0;
        for (index, screen, kind, step, doubled, lag) in transitions {
            let (mut this_rig, part) = at_screen(screen - 1);
            let display = if kind == "open" {
                Display::opening(part.open[step])
            } else {
                Display::closing(part.close[step].0)
            };
            let frame = render_at(&mut this_rig, display);
            let expected = byte_for(index);
            let differs = differing_in(&frame, &expected, 0, 200);
            assert_eq!(
                differs, 0,
                "{index} is not screen {screen}'s {kind} step {step}: {differs} bytes differ"
            );
            let port =
                PORT_TICK_ZERO + (screen as u32 - 1) * part.screen_ticks() + state_tick(kind, step);
            assert_eq!(
                index - port,
                lag,
                "{index}: screen {screen}'s {kind} step {step} is at {port}"
            );
            // And where the step's pan is not 0, the *register value* read as
            // pixels — the other reading of `(yy & 3) * 2` — is **not** the
            // frame: that is the measurement which fixes the pan's unit
            // (`the_pan_register_is_the_low_two_bits_of_yy_doubled`). The one
            // frame that does not discriminate (6340) is asserted to be exactly
            // that rather than skipped silently: at close step 48 the display
            // shows a sliver of the picture that is invariant under the
            // three-pixel shift.
            if let Some(doubled) = doubled {
                let mut wrong = display;
                wrong.pan *= 2;
                assert_ne!(display.pan, 0, "{index}: the doubled reading is the same frame");
                let differs = differing_in(&render_at(&mut this_rig, wrong), &expected, 0, 200);
                assert_eq!(differs, doubled, "{index}: the doubled reading");
                with_pan += 1;
                if doubled > 0 {
                    ruled_out += 1;
                }
            } else {
                assert_eq!(display.pan, 0, "{index}: a pan of 0 is the only case with one reading");
            }
        }
        // Nineteen of the twenty pan-carrying frames rule the doubled reading out; the
        // twentieth is the invariant sliver.
        assert_eq!(
            (with_pan, ruled_out),
            (20, 19),
            "the frames that measure the pan's unit"
        );

        // Two of the open frames are two steps' frame each: at 4940 open steps 48 and 49 of
        // screen 11, and at 7140 open steps 50 and 51 of screen 17, show the same pixels. The
        // table names the step that puts the open frame 300 ticks before its close partner, as
        // the other twelve pairs are.
        for (screen, kind, a, b) in [(11usize, "open", 48usize, 49usize), (17, "open", 50, 51)] {
            let (mut rig, part) = at_screen(screen - 1);
            assert_eq!(kind, "open");
            let first = render_at(&mut rig, Display::opening(part.open[a]));
            let second = render_at(&mut rig, Display::opening(part.open[b]));
            assert_eq!(first, second, "screen {screen}'s open steps {a} and {b}");
        }
        // The record's open frame and its close partner are 300 frames apart on every screen,
        // and the port's two states are 300 or 301 ticks apart: inside a screen the lag moves by
        // at most the one frame of a session's phase (screens 5 and 8).
        for pair in transitions.chunks_exact(2) {
            let (open, close) = (pair[0], pair[1]);
            assert_eq!((open.1, open.2, close.2), (close.1, "open", "close"));
            assert_eq!(close.0 - open.0, 300, "screen {}'s record pair", open.1);
            let gap = state_tick("close", close.3) - state_tick("open", open.3);
            assert!(gap == 300 || gap == 301, "screen {}'s pair is {gap} ticks apart", open.1);
            assert_eq!(open.5 - close.5, gap - 300, "screen {}'s two lags", open.1);
        }

        // **The lag grows by 3 to 4 frames a screen**, from 14 at screen 1 to 77 at screen 20:
        // `(77 - 14) / 19` = 3.3. Inside a screen it does not grow (above), so the release's
        // extra frames are spent between one screen's last close step and the next screen's
        // first open step, where `screenin` clears, uploads the palette and draws the caption
        // and the picture one `tw_putpixel` far call a pixel. The image's loops are the
        // tree's (102, 200 and 59 steps); the extra is CPU time, which the port's tick map does
        // not model (A11, A53).
        let lags: Vec<(usize, u32)> = transitions.iter().step_by(2).map(|t| (t.1, t.5)).collect();
        assert_eq!(
            lags,
            [
                (1, 14),
                (2, 18),
                (4, 24),
                (5, 28),
                (7, 33),
                (8, 37),
                (10, 43),
                (11, 47),
                (13, 54),
                (14, 57),
                (16, 64),
                (17, 67),
                (19, 74),
                (20, 77)
            ]
        );

        // **The sweep's 77 frames, accounted for.** §20.1's table names the hold
        // samples of every screen — 49 frames, every one this port's hold of
        // its screen (`the_records_hold_samples_are_the_releases_screens`) —
        // and the other 28 are the wipe states above, none of which is any
        // screen's hold.
        let holds: Vec<Vec<u8>> = (0..SCREEN_COUNT)
            .map(|screen| {
                let (mut rig, _) = at_screen(screen);
                render_at(&mut rig, hold_state())
            })
            .collect();
        let samples = frames.iter().filter(|(_, f)| holds.contains(f)).count();
        assert_eq!(samples, 49, "the sweep's hold samples");
        for (index, ..) in transitions {
            let bytes = byte_for(index);
            for (screen, hold) in holds.iter().enumerate() {
                assert_ne!(&bytes, hold, "{index} is screen {}'s hold", screen + 1);
            }
        }
        assert_eq!(samples + transitions.len(), frames.len());
    }

    #[test]
    #[ignore = "reads the scratch capture out/b5-s5-cred-map; a record, not an input"]
    fn the_records_hold_samples_are_the_releases_screens() {
        // A10 measured, against these samples, that nine of the 21 screens were not the tree's.
        // With the screens read from the image, every one of §20.1's 49 hold samples is the
        // port's hold of its own screen, 0 of 192,000 bytes — and of no other screen.
        //
        // `(screen, the number of the record's hold samples for it)`.
        let Some(frames) = record_frames() else {
            panic!(
                "this ignored test's input is the scratch capture `out/b5-s5-cred-map`, \
                 which is not here: there is nothing to check. Produce it with the sweep \
                 `reference-indices.md` §20 records (`cargo xtask reference capture`), or run \
                 the suite without `--ignored` — a missing capture must fail loudly, not \
                 pass silently"
            )
        };
        let samples: [usize; 21] = [
            2, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3, 2, 2, 3,
        ];
        let mut total = 0;
        let mut matched = Vec::new();
        for (screen, expected) in samples.iter().enumerate() {
            let (mut rig, _) = at_screen(screen);
            let hold = render_at(&mut rig, hold_state());
            let indices: Vec<u32> = frames
                .iter()
                .filter(|(_, f)| *f == hold)
                .map(|(at, _)| *at)
                .collect();
            assert_eq!(
                indices.len(),
                *expected,
                "screen {}'s hold samples: {indices:?}",
                screen + 1
            );
            // A screen's samples are one run of its hold, 100 frames apart.
            assert!(
                indices.windows(2).all(|pair| pair[1] - pair[0] == 100),
                "{indices:?}"
            );
            total += indices.len();
            matched.extend(indices);
        }
        assert_eq!(total, 49);
        matched.sort();
        matched.dedup();
        assert_eq!(matched.len(), 49, "no sample is two screens' hold");
        // The three keyframes A10 registered are among them: 3240 (screen 6), 5440 (12) and
        // 8740 (21), and 1440 (1) and 7640 (18) with them.
        for keyframe in [1440, 3240, 5440, 7640, 8740] {
            assert!(matched.contains(&keyframe), "{keyframe}");
        }
    }

    #[test]
    fn the_eight_rows_the_routine_never_reads_do_not_move_a_run() {
        // `MAIN.C:29` reads 32 rows of a raster that has 40. The scan is over
        // rows 0-31, and this is the measurement that says that cannot change
        // anything the part draws *but* the pixels themselves: no column is
        // inked in rows 32-39 that rows 0-31 leave blank, so all 47 runs — and
        // every `fonap`/`fonaw` — are the same over 32 rows as over 40.
        //
        // The eight rows hold 92 ink pixels, so the drawn difference is those
        // pixels, and the display shows every other logical line: 46 of the 92
        // fall on the rows a caption displays. `APPROXIMATIONS.md` A8.
        let (_, part) = started();
        let strip = part.strip.clone();
        let order = part.order.clone();
        let mut ap32 = [0u16; 256];
        let mut aw32 = [0u8; 256];
        scan_metrics(&strip, &order, &mut ap32, &mut aw32);

        let inked = |rows: std::ops::Range<usize>, x: usize| {
            rows.clone().any(|y| strip[y * FONT_WIDTH + x] != 0)
        };
        let mut ap40 = [0u16; 256];
        let mut aw40 = [0u8; 256];
        {
            // The same scan over all 40 rows.
            let mut x = 0usize;
            for &ch in &order {
                while x < FONT_WIDTH && !inked(0..40, x) {
                    x += 1;
                }
                if x >= FONT_WIDTH {
                    break;
                }
                let start = x;
                while x < FONT_WIDTH && inked(0..40, x) {
                    x += 1;
                }
                ap40[usize::from(ch)] = start as u16;
                aw40[usize::from(ch)] = (x - start) as u8;
            }
        }
        for &ch in &order {
            assert_eq!(
                ap32[usize::from(ch)],
                ap40[usize::from(ch)],
                "{:?}",
                ch as char
            );
            assert_eq!(
                aw32[usize::from(ch)],
                aw40[usize::from(ch)],
                "{:?}",
                ch as char
            );
        }
        let low: std::collections::BTreeSet<usize> =
            (0..FONT_WIDTH).filter(|x| inked(0..32, *x)).collect();
        let high: std::collections::BTreeSet<usize> =
            (0..FONT_WIDTH).filter(|x| inked(0..40, *x)).collect();
        assert_eq!(low, high, "no column is inked only in the rows 32-39");
        assert_eq!(low.len(), 447, "the strip's inked columns");

        let unread: usize = (32..40)
            .flat_map(|y| (0..FONT_WIDTH).map(move |x| (x, y)))
            .filter(|(x, y)| strip[y * FONT_WIDTH + x] != 0)
            .count();
        assert_eq!(unread, 92);
        let displayed: usize = (32..40)
            .flat_map(|y| (0..FONT_WIDTH).map(move |x| (x, y)))
            .filter(|(x, y)| *y % 2 == 0 && strip[y * FONT_WIDTH + x] != 0)
            .count();
        assert_eq!(displayed, 46, "the half of them a caption's rows show");
    }
}
