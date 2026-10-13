//! CRED — the 21 credit screens: a picture, a centred caption, and an iris.
//!
//! `CREDITS/MAIN.C` (318 lines) is the part itself, over `CREDITS/TWEAK.ASM`
//! (357 lines — the `tw_*` library PAM and ALKU share, whose six entry points
//! this port keeps together and named after the original) and the 21 packed
//! pictures `CREDITS/PICS/PIC*.LBM`. Its acts, in order:
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
//! # The display state a hold shows, and what is left for Task 8
//!
//! The parked state (`:165-167`) is `tw_setsplit(400)` — line compare 400, below
//! the bottom of a 200-row display, so inert — `tw_clrscr`, and
//! `tw_setstart(160*200)`: 32,000 bytes = 400 logical lines = 128,000 pixels.
//!
//! The **iris** is three registers stepped once a vertical blank — the line
//! compare (`tw_setsplit`), the display start (`tw_setstart`) and the AC's pan.
//! This task implements the two loops' *sequences* ([`open_ramp`],
//! [`close_ramp`]) and so their lengths, because the reference's keyframes are
//! holds at fixed indices and their positions are what those lengths put there;
//! and it implements the state the open loop's **last** step leaves
//! ([`HOLD_ORIGIN`], [`HOLD_LINE_COMPARE`], [`HOLD_PAN`]), which is the state
//! every frame of a hold shows. **The per-step register writes — and so every
//! frame of the two wipes, and the pan at any frame but the hold's — are
//! Task 8's**: in this commit the display sits at the parked state through the
//! open loop and at the hold's state from the loop's last step on. Nothing else
//! is deferred.
//!
//! # The three things worth knowing before changing anything here
//!
//! - **The font strip is not loaded through [`sr_assets::font`].** The routine
//!   states a 32-row cell (`MAIN.C:29`) and the raster has **40** rows, and it
//!   states a 32-column cell (`:270`: `fonap[32] = 1500-32`) on a strip 1,500
//!   columns wide: `1500/32 = 46.875`, so `Font::from_image` refuses it. The
//!   strip has no cell at all — the routine *scans* for ink runs (`:250-269`)
//!   and indexes the raster by column — so the part reads the raster itself.
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

use std::path::Path;

use sr_assets::load_path;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::{Split, View};
use sr_engine::{Palette, Surface};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "cred";

// ---------------------------------------------------------------- the font --

/// The font strip, read in its original format. `MAIN.C:31`'s
/// `char far font[FONAY][1500]` is an `INCBIN` of this file
/// (`CREDITS/INCLUDE.ASM`'s `asdf` segment): 60,000 `db` values, which is the
/// 1,500×40 raster `docs/fidelity/asset-inventory.md` §E.8 and §E.20 measure,
/// and byte for byte the raster `CREDITS/INCLUDE.OBJ` links.
const FONT_PATH: &str = "assets/CREDITS/FONA.INC";

/// The strip's raster width: `font[y][x]` is `FONT_WIDTH * y + x`, because the
/// source declares the symbol `[32][1500]` and so strides by the strip's own
/// 1,500 columns.
const FONT_WIDTH: usize = 1500;
/// `#define FONAY 32` (`MAIN.C:29`): the rows the routine reads, of a raster
/// that has 40. The eight it never reads hold 92 inked pixels, and none of them
/// is in a column the first 32 rows leave blank — so the metrics are the same
/// either way, and only those 92 pixels can differ. `APPROXIMATIONS.md` A8.
const FONT_ROWS: usize = 32;
/// The character order `init` scans the strip into (`MAIN.C:32`): 47 characters,
/// one per ink run the strip holds.
const FONT_ORDER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/?!:,\".()+-";
/// `fonap[32] = 1500-32` (`MAIN.C:270`): the blank's column.
const BLANK_COLUMN: u16 = (FONT_WIDTH - 32) as u16;
/// `fonaw[32] = 8` (`MAIN.C:271`): the blank's width — the routine's own, and
/// not a run the scan can find (the blank's columns hold no ink).
const BLANK_WIDTH: u8 = 8;
/// The blank's character: `fonap[32]`/`fonaw[32]` are indexed by the string's
/// bytes, and 32 is the space.
const BLANK: u8 = b' ';

/// Where a caption line is centred horizontally (`MAIN.C:172`: `prtc(160,y,text)`)
/// and where its first line starts.
const TEXT_CENTRE: usize = 160;
const TEXT_FIRST_LINE: usize = 16;
/// `y += FONAY+10` (`MAIN.C:172`): the step between caption lines.
const TEXT_LINE_STEP: usize = FONT_ROWS + 10;

// ------------------------------------------------------------ the pictures --

/// Where the pictures live, and their geometry: `screenin`'s
/// `char (*pic)[160]`, and `:174`'s 160×100 loop.
const PICTURE_DIR: &str = "assets/CREDITS/PICS";
const PICTURE_WIDTH: usize = 160;
const PICTURE_HEIGHT: usize = 100;
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
const HOLD_ORIGIN: u32 = PARKED_ORIGIN + LINE_LEN as u32;
/// The hold's line compare (`MAIN.C:179`: `y/128 + 200` with `y = 1`): 200.
const HOLD_LINE_COMPARE: u16 = 200;
/// The pan the hold shows (`:183-191` with `yy = 320`): `(320 & 3) * 2 = 0`.
const HOLD_PAN: u8 = 0;

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

/// The 21 screens, in `MAIN.C:41-156`'s order: the picture's file and the
/// caption's lines. A caption line is one of the source's string literals,
/// which end at a NUL, so an empty entry is a line that draws nothing and still
/// advances `y` (`:172`).
const SCREENS: &[(&str, &[&str])] = &[
    ("PIC01.LBM", &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - WILDFIRE"]),
    ("PIC02.LBM", &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - PSI"]),
    (
        "PIC03.LBM",
        &["GRAPHICS - MARVEL", "MUSIC - SKAVEN", "CODE - WILDFIRE", "ANIMATION - TRUG"],
    ),
    ("PIC04.LBM", &["", "GRAPHICS - PIXEL"]),
    ("PIC05.LBM", &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC05B.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - TRUG"]),
    ("PIC06.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC07.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC08.LBM", &["", "GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION"]),
    (
        "PIC09.LBM",
        &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - TRUG", "RENDERING - TRUG"],
    ),
    ("PIC10.LBM", &["GRAPHICS - PIXEL, SKAVEN", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC10B.LBM", &["GRAPHICS - PIXEL, SKAVEN", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC11.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"]),
    ("PIC12.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"]),
    ("PIC13.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    (
        "PIC14.LBM",
        &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - TRUG", "RENDERING - TRUG"],
    ),
    ("PIC14B.LBM", &["", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC15.LBM", &["GRAPHICS - MARVEL", "MUSIC - PURPLE MOTION", "CODE - PSI"]),
    ("PIC16.LBM", &["", "MUSIC - SKAVEN", "CODE - PSI"]),
    ("PIC17.LBM", &["", "GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION"]),
    ("PIC18.LBM", &["GRAPHICS - PIXEL", "MUSIC - PURPLE MOTION", "CODE - WILDFIRE"]),
];

/// One screen: its picture, the palette the screen uploads, and its caption.
struct Screen {
    /// The picture's pixels, row-major at [`PICTURE_WIDTH`].
    picture: Vec<u8>,
    /// The picture's palette *after* `init`'s repair — what `tw_setpalette`
    /// uploads.
    palette: Palette,
    /// The caption's lines, in `MAIN.C`'s order.
    text: &'static [&'static str],
}

/// CRED: the strip, the 21 screens, and where in a screen the part's tick is.
pub struct Cred {
    /// The font raster, row-major at [`FONT_WIDTH`] — all 40 rows of the file,
    /// of which the routine reads [`FONT_ROWS`].
    strip: Vec<u8>,
    /// `fonap` (`MAIN.C:33`): the column each character's ink starts at.
    fonap: [u16; 256],
    /// `fonaw` (`MAIN.C:34`): each character's ink width, and the routine's 8
    /// for the blank.
    fonaw: [u8; 256],
    /// The 21 screens in `MAIN.C`'s order. Empty until `init`.
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
    /// Where the font and the pictures are read from.
    font_path: &'static str,
    pictures_path: &'static str,
}

impl Cred {
    /// A part with nothing loaded: `init` is what reads the strip and the 21
    /// pictures.
    pub fn new() -> Cred {
        Cred {
            strip: Vec::new(),
            fonap: [0; 256],
            fonaw: [0; 256],
            screens: Vec::new(),
            open: open_ramp(),
            close: close_ramp(),
            screen: 0,
            tick: 0,
            font_path: FONT_PATH,
            pictures_path: PICTURE_DIR,
        }
    }

    /// [`new`](Cred::new) reading the strip and the pictures from elsewhere.
    ///
    /// A unit test's working directory is this crate's, not the repository
    /// root, so the part's own relative paths do not resolve there; the
    /// integration test in `tests/cred.rs` runs the real binary from the
    /// repository root, where they do.
    #[cfg(test)]
    fn at(font: &'static str, pictures: &'static str) -> Cred {
        Cred {
            font_path: font,
            pictures_path: pictures,
            ..Cred::new()
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
        self.screen_ticks() * SCREENS.len() as u32
    }

    /// `MAIN.C:169-174` — the palette, the caption, then the picture.
    fn show_screen(&self, ctx: &mut PartCtx) {
        let screen = &self.screens[self.screen];
        ctx.set_palette(screen.palette);
        self.draw_text(ctx, screen.text);
        self.draw_picture(ctx, &screen.picture);
    }

    /// `MAIN.C:172` — one centred line per entry of `text`, `FONAY+10` logical
    /// lines apart. The source's `while(*(text = prtc(...)))` stops at the
    /// terminating empty literal, which draws nothing and does not advance `y`,
    /// so the entries here are exactly the lines it draws.
    fn draw_text(&self, ctx: &mut PartCtx, text: &[&str]) {
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
    fn draw_centred(&self, ctx: &mut PartCtx, x: usize, y: usize, text: &str) {
        let width: usize = text
            .bytes()
            .map(|ch| usize::from(self.width(ch)) + 2)
            .sum();
        self.draw_line(ctx, x - width / 2, y, text);
    }

    /// `MAIN.C:215-234` — `prt`: for each character, `fonaw[ch]` columns
    /// starting at `fonap[ch]`, `FONAY` rows each, then a two-column gap.
    fn draw_line(&self, ctx: &mut PartCtx, mut x: usize, y: usize, text: &str) {
        for ch in text.bytes() {
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

    /// The display, as the view the port holds: `MAIN.C:165-167`'s parked state
    /// and `:176-182`'s last open step.
    fn set_display(ctx: &mut PartCtx, parked: bool) {
        let (origin, line_compare, pan) = if parked {
            (PARKED_ORIGIN, PARKED_LINE_COMPARE, 0)
        } else {
            (HOLD_ORIGIN, HOLD_LINE_COMPARE, HOLD_PAN)
        };
        // `TWEAK.ASM:18-60`: 320 pixels a row, 200 rows, a 160-byte line offset
        // and no separate scan doubling — the doubling the CRTC asks for is
        // what the 160-byte offset already folds in, which is the arithmetic
        // this module's docs spell out.
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.y_line_offset = Some(DISPLAY_LINE_STRIDE);
        ctx.view.double_scan = false;
        ctx.view.pan = pan;
        ctx.view.set_origin(origin);
        // The lower view reads from origin 0 with the surface's own stride
        // (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`): the hardware sets the
        // address counter to 0 and clears panning at the split, and the line
        // offset register is unchanged — 160 bytes, the same 640 pixels the
        // surface's own rows are.
        ctx.view.split = Some(Box::new(Split {
            scanline: split_scanline(line_compare),
            view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
        }));
    }
}

impl Part for Cred {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:38-39`'s `init()` and `tw_opengraph`, plus the font and the 21
    /// pictures.
    ///
    /// The metrics scan (`:250-269`), the pictures with their palettes repaired
    /// (`:273-317`), and `tw_opengraph`'s clear (`TWEAK.ASM:29-34`) — the whole
    /// 256 KB, so the port's whole surface. The CRTC and AC state
    /// `tw_opengraph` writes is the view, which is set per screen.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        self.strip = parse_strip(Path::new(self.font_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.font_path),
        })?;
        scan_metrics(&self.strip, &mut self.fonap, &mut self.fonaw);

        self.screens = Vec::with_capacity(SCREENS.len());
        for (file, text) in SCREENS {
            let path = format!("{}/{file}", self.pictures_path);
            let image = load_path(Path::new(&path)).map_err(|error| PartError {
                part: NAME,
                message: format!("{path}: {error}"),
            })?;
            let (width, height) = (image.width() as usize, image.height() as usize);
            if (width, height) != (PICTURE_WIDTH, PICTURE_HEIGHT) {
                return Err(PartError {
                    part: NAME,
                    message: format!(
                        "{path} is {width}x{height}; this part's pictures are \
                         {PICTURE_WIDTH}x{PICTURE_HEIGHT}"
                    ),
                });
            }
            let mut palette = image.palette;
            repair_palette(&mut palette);
            self.screens.push(Screen {
                picture: image.pixels().to_vec(),
                palette,
                text,
            });
        }

        // `TWEAK.ASM:29-34`: 32,768 word writes with the map mask open, which
        // is 65,536 byte addresses in each of the four planes — every pixel the
        // machine has, and so the whole surface. Nothing a previous part left
        // can show through a credit screen.
        ctx.surface.clear(0);
        Cred::set_display(ctx, true);
        Ok(())
    }

    /// One tick of one screen. `MAIN.C:161-213`'s shape, one tick a vertical
    /// blank:
    ///
    /// - tick 0 (`:165-168`) — the parked split, `tw_clrscr`, the parked start,
    ///   then the blank.
    /// - tick 1 (`:169-174`) — the palette, the caption, the picture.
    /// - the next [`OPEN_STEPS`] ticks (`:176-192`) — the iris opening. The
    ///   loop's *length* is this task's; its register writes are Task 8's, so
    ///   the display stays parked until the loop's last step, whose state is
    ///   then the one every hold frame shows.
    /// - then [`HOLD_FRAMES`] ticks (`:194`) — the hold, with nothing written.
    /// - then [`CLOSE_STEPS`] ticks (`:196-212`) — the iris closing, after
    ///   which the next screen's tick 0 re-parks the display.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let tick = self.tick;
        let open = self.open.len() as u32;
        if tick >= self.screen_ticks() {
            return PartStatus::Finished;
        }
        match tick {
            PARKED_TICK => {
                Cred::set_display(ctx, true);
                // `TWEAK.ASM:62-75`: 1,000h dwords = 65,536 pixels, *not* the
                // whole surface — the picture band is past the clear and is
                // overwritten by this screen's own picture.
                for pixel in &mut ctx.surface.pixels_mut()[..CLRSCR_PIXELS] {
                    *pixel = 0;
                }
            }
            SHOW_TICK => self.show_screen(ctx),
            tick if tick == SHOW_TICK + open => {
                // The open loop's last step (`:176`'s last iteration, whose
                // `dis_waitb` is what shows it): tick 2 is the first step's and
                // tick `SHOW_TICK + open` = 103 the last's, so the hold's state
                // is displayed from 103 — the first of the hold's 200 frames is
                // 104, and the close's first is 304.
                Cred::set_display(ctx, false);
            }
            _ => {}
        }

        self.tick += 1;
        if self.tick == self.screen_ticks() {
            self.tick = 0;
            self.screen += 1;
            if self.screen == SCREENS.len() {
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
fn scan_metrics(strip: &[u8], fonap: &mut [u16; 256], fonaw: &mut [u8; 256]) {
    let inked = |x: usize| (0..FONT_ROWS).any(|y| strip[y * FONT_WIDTH + x] != 0);
    let mut x = 0usize;
    for ch in FONT_ORDER.bytes() {
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

/// Read `FONA.INC` into its raster: the strip as the original stored it.
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

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::palette::PaletteTimeline;

    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// The paths a unit test reads: this crate's own directory, two levels
    /// below the repository root, so `assets/` is not where the part's own
    /// relative path looks.
    const TEST_FONT: &str = "../../assets/CREDITS/FONA.INC";
    const TEST_PICTURES: &str = "../../assets/CREDITS/PICS";

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
                frame,
                0,
                1,
                true,
                false,
            )
        }
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Cred) {
        let mut rig = Rig::new();
        let mut part = Cred::at(TEST_FONT, TEST_PICTURES);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Cred, ticks: u32) {
        for at in 0..ticks {
            assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Continuing, "tick {at}");
        }
    }

    /// The frame the reference holds, read from the tree it is committed in.
    fn reference(name: &str) -> Vec<u8> {
        std::fs::read(format!("../../reference/parts/cred/{name}")).unwrap()
    }

    /// The port's displayed frame as RGB, one pixel per displayed pixel, through
    /// the live palette and the engine's own 6→8-bit widening.
    fn rendered(rig: &Rig) -> Vec<u8> {
        let mut out = Vec::with_capacity(VIEW_WIDTH as usize * VIEW_HEIGHT as usize * 3);
        for y in 0..VIEW_HEIGHT {
            for x in 0..VIEW_WIDTH {
                let at = rig.view.source_index(&rig.surface, x, y);
                let entry = rig.palette.entry(rig.surface.pixels()[at]);
                out.push(sr_engine::dac_expand(entry[0]));
                out.push(sr_engine::dac_expand(entry[1]));
                out.push(sr_engine::dac_expand(entry[2]));
            }
        }
        out
    }

    /// The width of a caption line, by the source's own arithmetic.
    fn line_width(part: &Cred, line: &str) -> usize {
        line.bytes().map(|ch| usize::from(part.width(ch)) + 2).sum()
    }

    #[test]
    fn the_strip_parses_to_the_raster_the_source_declares() {
        // `MAIN.C:31`: `char far font[FONAY][1500]` — 60,000 `db` values, 40
        // rows of 1,500, every value a 0-9 shade.
        let strip = parse_strip(Path::new(TEST_FONT)).unwrap();
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
    fn the_metrics_are_the_runs_the_strip_itself_holds() {
        // `MAIN.C:250-269`: one ink run per `fonaorder` character, in order —
        // 47 runs for 47 characters, 447 inked columns, the last ink at column
        // 557.
        let (_, part) = started();
        let mut inked_columns = 0usize;
        for ch in FONT_ORDER.bytes() {
            let width = part.fonaw[usize::from(ch)];
            assert!(width > 0, "{:?} got no run", ch as char);
            let start = usize::from(part.fonap[usize::from(ch)]);
            assert!((0..FONT_ROWS).any(|y| part.strip[y * FONT_WIDTH + start] != 0));
            inked_columns += usize::from(width);
        }
        assert_eq!(FONT_ORDER.len(), 47);
        assert_eq!(inked_columns, 447, "the strip's inked columns");
        let widths: Vec<usize> = FONT_ORDER
            .bytes()
            .map(|ch| usize::from(part.width(ch)))
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
        assert_eq!(part.width(b'.'), 3, "the full stop is the widest of the marks");
        assert_eq!(part.fonap[usize::from(b'-')], 548, "the last of the 47 runs");
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
        let line = "GRAPHICS - MARVEL";
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
        let second = SCREENS[0].1[1];
        let second_start = TEXT_CENTRE - line_width(&part, second) / 2;
        assert!(second_start > start, "a narrower line starts further right");
        assert_eq!(
            rig.surface.pixels()[(TEXT_FIRST_LINE + TEXT_LINE_STEP) * LINE_LEN + second_start],
            part.strip[usize::from(part.column(b'M'))],
            "the second line starts 42 logical lines below the first"
        );
    }

    #[test]
    fn the_sixth_screen_draws_pic5b_and_it_is_its_own_picture() {
        // `MAIN.C:64-73`: screens 5 and 6 are `pic5` and `pic5b` — two files,
        // and two pictures: they agree on 6 of their 16,000 pixels, and
        // `PIC05B.LBM` sets 43 of its palette's entries against `PIC05.LBM`'s
        // 239. The second is not a crop or a repeat of the first, which is what
        // this checks before drawing it.
        assert_eq!(SCREENS[4].0, "PIC05.LBM");
        assert_eq!(SCREENS[5].0, "PIC05B.LBM");
        assert_eq!(SCREENS[5].1, &["", "MUSIC - PURPLE MOTION", "CODE - TRUG"]);
        let (mut rig, mut part) = started();
        let fifth = part.screens[4].picture.clone();
        let sixth = part.screens[5].picture.clone();
        assert_eq!(fifth.len(), PICTURE_WIDTH * PICTURE_HEIGHT);
        assert_eq!(sixth.len(), PICTURE_WIDTH * PICTURE_HEIGHT);
        let agreeing = fifth.iter().zip(&sixth).filter(|(a, b)| a == b).count();
        assert_eq!(agreeing, 6, "the two pictures share {agreeing} of 16,000 pixels");
        assert!(sixth.iter().copied().max() == Some(43), "pic5b's highest index");

        // And screen 6's own picture is what the screen holds, at `+16`.
        // Five screens in, on the tick after screen 6's parked one: the tick
        // its palette and its picture land on.
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
        assert_eq!(SCREENS.len(), 21, "`MAIN.C:41-156`'s calls");
        let (mut rig, mut part) = started();
        assert_eq!(part.screen_ticks(), 363);
        assert_eq!(part.total_ticks(), 7_623);
        let last = part.total_ticks() - 1;
        run_ticks(&mut rig, &mut part, last);
        // The last tick draws the last screen's last frame *and* ends `main()`:
        // the shell composes that tick's frame and hands the next one to
        // ENDSCRL, exactly as the original's loader does.
        assert_eq!(part.frame(&mut rig.ctx(last)), PartStatus::Finished);
        assert_eq!(part.screen, SCREENS.len());
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
        // repair, the font and the display state at once: the port's screen 1
        // and screen 18 are `reference/parts/cred`'s frames at 1440 and 7640,
        // byte for byte.
        //
        // It is also what fixes `HOLD_ORIGIN`: at the *parked* origin the
        // display reads the even logical lines, which hold no picture, and
        // every one of the picture's 96,000 bytes differs.
        let (mut rig, mut part) = started();
        let hold_first = SHOW_TICK + OPEN_STEPS + 1;

        run_ticks(&mut rig, &mut part, hold_first);
        let first = rendered(&rig);
        let at_1440 = reference("frame_001440.rgb");
        assert_eq!(first.len(), at_1440.len());
        let differing = first.iter().zip(&at_1440).filter(|(a, b)| a != b).count();
        assert_eq!(differing, 0, "screen 1 differs from the reference in {differing} bytes");

        // The parked state is not the hold's: the same frame drawn from the
        // parked origin is a different picture.
        let hold_view = rig.view.clone();
        Cred::set_display(&mut rig.ctx(hold_first), true);
        let parked = rendered(&rig);
        let parked_differing = parked.iter().zip(&at_1440).filter(|(a, b)| a != b).count();
        assert_eq!(
            parked_differing, 29_523,
            "the parked display reads the even logical lines, which hold no picture; it differs \
             from the reference's frame in {parked_differing} bytes"
        );
        rig.view = hold_view;

        // Screen 18: seventeen screens further on, the same claim.
        let ticks = 17 * part.screen_ticks();
        run_ticks(&mut rig, &mut part, ticks);
        let eighteenth = rendered(&rig);
        let at_7640 = reference("frame_007640.rgb");
        let differing = eighteenth.iter().zip(&at_7640).filter(|(a, b)| a != b).count();
        assert_eq!(differing, 0, "screen 18 differs from the reference in {differing} bytes");
    }

    #[test]
    fn the_references_sixth_screen_is_not_the_trees_picture_at_all() {
        // Screen 6's keyframe is 3240, and the port's caption there is the
        // tree's — the difference is entirely in the picture band. This is the
        // measurement that says the *shipped* build's picture is not the
        // tree's, rather than the port's rendering being wrong: the reference's
        // band is drawn in colours this screen's own palette does not contain.
        //
        // The palette is the one `tw_setpalette` uploads (the picture's, after
        // the repair), and the reference's band has 14 distinct colours, of
        // which 8 — 3,524 of the band's 16,000 pixels — are in none of its 256
        // entries. The tree's `PIC05B.LBM` is a 44-index noise whose greys are
        // not in the palette; the shipped screen shows a greyscale starburst,
        // which is what `reference-indices.md` §20.1's own sheet description of
        // screen 6 ("a dotted vortex") describes.
        let (mut rig, mut part) = started();
        // Five screens in, on the tick screen 6's picture is drawn on, and at
        // the display state the hold shows (the one the frames are read at).
        let ticks = 5 * part.screen_ticks() + SHOW_TICK + OPEN_STEPS;
        run_ticks(&mut rig, &mut part, ticks);

        let mut palette = std::collections::BTreeSet::new();
        for index in 0u8..=255 {
            let entry = rig.palette.entry(index);
            palette.insert([
                sr_engine::dac_expand(entry[0]),
                sr_engine::dac_expand(entry[1]),
                sr_engine::dac_expand(entry[2]),
            ]);
        }
        let reference = reference("frame_003240.rgb");
        // The band: displayed rows 0-99, columns 80-239 — where the picture is.
        let mut band_colours = std::collections::BTreeMap::new();
        for y in 0..100usize {
            for x in 80..240usize {
                let at = (y * 320 + x) * 3;
                let rgb = [reference[at], reference[at + 1], reference[at + 2]];
                *band_colours.entry(rgb).or_insert(0usize) += 1;
            }
        }
        let outside: Vec<([u8; 3], usize)> = band_colours
            .iter()
            .filter(|(rgb, _)| !palette.contains(*rgb))
            .map(|(rgb, count)| (*rgb, *count))
            .collect();
        assert_eq!(band_colours.len(), 14, "the reference band's distinct colours");
        assert_eq!(outside.len(), 8, "of which the screen's palette has none: {outside:?}");
        assert_eq!(
            outside.iter().map(|(_, count)| count).sum::<usize>(),
            3_524,
            "the pixels drawn in a colour this screen's palette does not have"
        );
        // The greys the tree's picture could not have produced: the ramp is the
        // only neutral sequence in this palette, and none of these is on it.
        assert!(outside.iter().all(|(rgb, _)| rgb[0] == rgb[1] && rgb[1] == rgb[2]));

        // And the port's own band is the tree's picture: every pixel of it is
        // a colour the palette has.
        let mine = rendered(&rig);
        for at in (0..mine.len()).step_by(3) {
            let rgb = [mine[at], mine[at + 1], mine[at + 2]];
            assert!(palette.contains(&rgb), "{rgb:?} is not in the screen's palette");
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
        let strip = parse_strip(Path::new(TEST_FONT)).unwrap();
        let mut ap32 = [0u16; 256];
        let mut aw32 = [0u8; 256];
        scan_metrics(&strip, &mut ap32, &mut aw32);

        let inked = |rows: std::ops::Range<usize>, x: usize| {
            rows.clone().any(|y| strip[y * FONT_WIDTH + x] != 0)
        };
        let mut ap40 = [0u16; 256];
        let mut aw40 = [0u8; 256];
        {
            // The same scan over all 40 rows.
            let mut x = 0usize;
            for ch in FONT_ORDER.bytes() {
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
        for ch in FONT_ORDER.bytes() {
            assert_eq!(ap32[usize::from(ch)], ap40[usize::from(ch)], "{:?}", ch as char);
            assert_eq!(aw32[usize::from(ch)], aw40[usize::from(ch)], "{:?}", ch as char);
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
