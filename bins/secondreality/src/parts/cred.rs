//! CRED — the 21 credit screens: a picture, a centred caption, and an iris.
//!
//! The code is `CREDITS/MAIN.C` over `CREDITS/TWEAK.ASM` (the `tw_*` library
//! PAM and ALKU share); **the data is the shipped build's** (D49), read out of
//! `sr_assets::exe::unpack(assets/MAIN/DATA/CRED.EXE)`: `main`'s 21
//! `screenin(pic, text)` calls ([`SCREENIN_CALL`]) name each picture blob and
//! caption, and the font raster and `fonaorder` sit where `init` and `prt`
//! address them. The image's code is the tree's except for three things the
//! port does not model (`docs/fidelity/cred.md` §9). One tick is one
//! `dis_waitb`:
//!
//! ```text
//! init   :37-39, :246-318  init(): the font scan and the palette repair; tw_opengraph's clear
//! 0      :165-168          tw_setsplit(400), tw_clrscr, tw_setstart(160*200): parked
//! 1      :169-174          tw_setpalette, the caption (:172), the picture at +16 (:174)
//! 2-103  :176-192          the iris opens, y = y*12L/13 from 200*128: 102 steps
//! ...    :194              the hold: 200 waits, nothing written
//! ...    :196-212          the iris closes, y += v, v += 15: 59 steps; 363 ticks a screen
//! ```
//!
//! `tw_opengraph` (`TWEAK.ASM:18-60`) sets chain-4 off, CRTC 0x13 = `0x5013`
//! and CRTC 9 = `0x0109`: the pen (`tw_putpixel`, `:87-117`) writes a 320-pixel
//! logical line ([`LINE_LEN`]) while the display advances 160 bytes = 640
//! pixels a row, so a displayed row is every other logical line. The port
//! writes flat at 320 and reads with `y_line_offset = Some(640)` (D16). The
//! picture lands on the odd lines from 401, the caption on the even ones,
//! shown below the split where the address counter restarts at 0.
//!
//! Each wipe step writes three registers ([`Display`]). The `for(a=0;a<10000;a++)`
//! delay between them (`:182`, `:202`; a retrace wait in the image) is not
//! modelled: the tick is the frame boundary it protects. The release's screens
//! are 3 to 4 frames longer than 363 (CPU time spent drawing; A11, A53).
//!
//! The font strip is read raw, 32 of its 40 rows (A8), not as a
//! [`sr_assets::font`] `Font`.

use sr_assets::exe::{unpack, LoadImage};
use sr_assets::{load, Format};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::{Split, View};
use sr_engine::{Palette, Surface};

use crate::parts::read_asset;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "cred";

// --------------------------------------------------------------- the image --

/// The shipped part, PKLITE-packed (`docs/fidelity/release-images.md`); every
/// offset below is into its unpacked load image.
const EXE_PATH: &str = "assets/MAIN/DATA/CRED.EXE";
/// DGROUP: `mov dx,6752h` at image 0, ×16. The captions are near pointers
/// into it.
const DGROUP: usize = 0x6752 * 16;
/// `main` (`MAIN.C:36-159`), from its first instruction to `screenin` at
/// `0x1b69`, the `lcall 0190:0269` target.
const MAIN_AT: usize = 0x190c;
const MAIN_END: usize = 0x1b69;
/// One of `main`'s `screenin(pic, text)` calls: `push ds; push text; push
/// pic_seg; push 0; lcall 0190:0269`. `pic_seg` is relocated; `text` is a
/// DGROUP offset.
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
/// `MAIN.C:41-156`'s 21 screens.
const SCREEN_COUNT: usize = 21;

// ---------------------------------------------------------------- the font --

/// `char far font[FONAY][1500]` (`MAIN.C:31`): segment `0x29f` (`mov
/// dx,029fh` at `0x1e83`, `0x1ec5`, `0x1db0`).
const FONT_AT: usize = 0x29f * 16;
/// The row stride: `mov dx,05dch; imul dx` (`0x1e7b`).
const FONT_WIDTH: usize = 1500;
/// The rows the image links, up to the first picture (A8).
const FONT_ARRAY_ROWS: usize = 40;
/// `#define FONAY 32` (`MAIN.C:29`; `cmp word [bp-4],20h` at `0x1e95`): the
/// rows the routine reads (A8).
const FONT_ROWS: usize = 32;
/// The far pointer `fonaorder` (`MAIN.C:32`; `les bx,[0092h]` at `0x1eed`).
const FONAORDER_PTR: usize = DGROUP + 0x92;
/// `fonap[32] = 1500-32` and `fonaw[32] = 8` (`MAIN.C:270-271`; `0x1f30`,
/// `0x1f36`): the blank, which no ink run provides.
const BLANK_COLUMN: u16 = (FONT_WIDTH - 32) as u16;
const BLANK_WIDTH: u8 = 8;
const BLANK: u8 = b' ';

/// `MAIN.C:172`: `prtc(160,y,text)` from `y = 16`, `y += FONAY+10`.
const TEXT_CENTRE: usize = 160;
const TEXT_FIRST_LINE: usize = 16;
const TEXT_LINE_STEP: usize = FONT_ROWS + 10;

// ------------------------------------------------------------ the pictures --

/// `screenin`'s `char (*pic)[160]` and `:174`'s 160×100 loop.
const PICTURE_WIDTH: usize = 160;
const PICTURE_HEIGHT: usize = 100;
/// One `FC FC` blob: a 16-byte header, the palette at 16
/// (`tw_setpalette(&pic[0][16])`) and the pixels at 784; the 0x419-paragraph
/// step between `main`'s 21 segments.
const PICTURE_BYTES: usize = 784 + PICTURE_WIDTH * PICTURE_HEIGHT;
/// `:174`'s `tw_putpixel(400+x, 400+y*2, pic[y][x]+16)`.
const PICTURE_COLOUR_OFFSET: u8 = 16;
const PICTURE_FIRST_COLUMN: usize = 400;
const PICTURE_FIRST_LINE: usize = 400;
const PICTURE_LINE_STEP: usize = 2;

/// `init`'s palette repair (`MAIN.C:273-317`): the `memmove`'s extent and
/// shift, and the grey ramp's.
const REPAIR_COUNT: usize = 240;
const REPAIR_SHIFT: u8 = 16;
const RAMP_ENTRIES: u8 = 10;
const RAMP_STEP: u8 = 7;

// ------------------------------------------------------------- the screen --

/// The pen's logical line: `tw_putpixel`'s `y*80 + x/4` byte address holds
/// four pixels, one per plane (`TWEAK.ASM:104-110`).
const LINE_LEN: usize = 320;

/// `tw_clrscr` (`TWEAK.ASM:62-75`): 4,096 dwords = 16,384 byte addresses × 4
/// planes. The previous picture's band is past it and is overwritten.
const CLRSCR_PIXELS: usize = 16_384 * 4;

/// 320 pixels a row (CRTC 0x13 = `0x5013`) and 200 rows (CRTC 9 = `0x0109`),
/// 160 bytes = 640 pixels a displayed row.
const VIEW_WIDTH: u16 = 320;
const VIEW_HEIGHT: u16 = 200;
const DISPLAY_LINE_STRIDE: u16 = 640;

/// `tw_setstart(160*200)` (`MAIN.C:167`): 32,000 bytes = logical line 400.
const PARKED_ORIGIN: u32 = 160 * 200 * 4;
/// `tw_setsplit(400)` (`MAIN.C:165`): below a 200-row display, so inert.
const PARKED_LINE_COMPARE: u16 = 400;

/// What `tw_setstart` (`:167`, `:182`, `:202`) and `tw_setsplit` (`:179`,
/// `:199`) add to a step's `yy/4` and `y/128`.
const DISPLAY_START_BIAS: u16 = 160 * 200;
const LINE_COMPARE_BIAS: u16 = 200;

/// The first row below the split for a line-compare value: the register names
/// the last line above it, and the mode is double-scanned, so
/// `(line_compare + 1) / 2` (the emulator's `vga_draw.cpp:1586-1593`). The
/// hold's 200 lands the caption on row 100, which rediff's five keyframes pin.
const fn split_scanline(line_compare: u16) -> u16 {
    line_compare.div_ceil(2)
}

/// One iris step's three register writes, in the port's units
/// (`MAIN.C:179-191` opening, `:199-211` closing):
///
/// ```text
/// tw_setsplit(y/128 + 200)          the line compare
/// yy = 320 - y/80  (open)  320 + y/80  (close)
/// tw_setstart(160*200 + yy/4)       a byte address (CRTC 0x17 byte mode): ×4 pixels
/// AC 0x13 = (yy & 3) * 2            the pan
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Display {
    /// The display start, in pixels.
    origin: u32,
    /// The line-compare register, as `tw_setsplit` is given it.
    line_compare: u16,
    /// The pan in pixels, not the AC 0x13 value ([`pan`]).
    pan: u8,
}

impl Display {
    /// `MAIN.C:165-167`.
    const PARKED: Display = Display {
        origin: PARKED_ORIGIN,
        line_compare: PARKED_LINE_COMPARE,
        pan: 0,
    };

    const fn opening(y: u16) -> Display {
        Display::from_y(y, 320 - y / 80)
    }

    const fn closing(y: u16) -> Display {
        Display::from_y(y, 320 + y / 80)
    }

    const fn from_y(y: u16, yy: u16) -> Display {
        Display {
            origin: (DISPLAY_START_BIAS + yy / 4) as u32 * 4,
            line_compare: y / 128 + LINE_COMPARE_BIAS,
            pan: pan(yy),
        }
    }
}

/// AC 0x13's shift in pixels: half the register `(yy & 3) * 2`, because a
/// 256-colour mode pans by `(val & 7) / 2` (`vga_attr.cpp:225-227`) in the
/// display start's own unit (`vga_draw.cpp:1648-1651`).
const fn pan(yy: u16) -> u8 {
    (yy & 3) as u8
}

// ------------------------------------------------------------- the timing --

/// `MAIN.C:176`'s `for(y=200*128; y>0; y=y*12L/13)`, as its `y` values.
fn open_ramp() -> Vec<u16> {
    let mut y: u32 = 200 * 128;
    let mut steps = Vec::new();
    while y > 0 {
        steps.push(y as u16);
        y = y * 12 / 13;
    }
    steps
}

/// `MAIN.C:196`'s `for(y=0,v=0; y<128*200; y=y+v,v+=15)`, as its `y` values.
fn close_ramp() -> Vec<u16> {
    let (mut y, mut v): (u32, u32) = (0, 0);
    let mut steps = Vec::new();
    while y < 128 * 200 {
        steps.push(y as u16);
        y += v;
        v += 15;
    }
    steps
}

/// `MAIN.C:165-168`: the parked state, before the blank.
const PARKED_TICK: u32 = 0;
/// `MAIN.C:169-174`: the palette, the caption and the picture.
const SHOW_TICK: u32 = PARKED_TICK + 1;
/// `MAIN.C:194`'s `for(a=0;a<200 && !dis_exit();a++) dis_waitb()`.
const HOLD_FRAMES: u32 = 200;

/// One screen: its picture, the palette it uploads, and its caption.
struct Screen {
    /// The picture's pixels, row-major at [`PICTURE_WIDTH`].
    picture: Vec<u8>,
    /// The picture's palette after `init`'s repair.
    palette: Palette,
    /// The caption's lines; an empty one draws nothing and still advances `y`.
    text: Vec<Vec<u8>>,
}

/// One of `main`'s `screenin` calls, as the image holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ScreenCall {
    /// The call's image offset: its `push ds`.
    at: usize,
    /// `pic`'s segment: the picture is at image `segment * 16`.
    segment: u16,
    /// `text`'s offset in DGROUP.
    text: u16,
}

/// What `init` and `main` read out of the image.
struct ImageData {
    /// All [`FONT_ARRAY_ROWS`] rows.
    raster: Vec<u8>,
    /// `fonaorder`, up to its NUL.
    order: Vec<u8>,
    calls: Vec<ScreenCall>,
    pictures: Vec<Vec<u8>>,
    captions: Vec<Vec<Vec<u8>>>,
}

/// Every match of [`SCREENIN_CALL`] inside `main`, in order.
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

/// The caption at `at`: `MAIN.C:172`'s `while(*(text=prtc(160,y,text)))`
/// draws the first line even when empty and stops at a double NUL.
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

/// Read [`ImageData`] out of an unpacked image, refusing one whose layout is
/// not this build's.
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
    for call in &calls {
        let relocated = |at: usize| image.relocations.contains(&(at as u32));
        if !relocated(call.at + SCREENIN_PICTURE) || relocated(call.at + SCREENIN_TEXT) {
            return Err(format!(
                "the screenin call at {:#x} is not relocated as a far pic",
                call.at
            ));
        }
    }

    // The unrelocated segment word is relative to the image's origin.
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
    /// The font raster at [`FONT_WIDTH`], all 40 rows; the routine reads
    /// [`FONT_ROWS`].
    strip: Vec<u8>,
    /// `fonap` (`MAIN.C:33`): the column each character's ink starts at.
    fonap: [u16; 256],
    /// `fonaw` (`MAIN.C:34`): each character's ink width.
    fonaw: [u8; 256],
    /// The 21 screens in `main`'s order. Empty until `init`.
    screens: Vec<Screen>,
    /// [`open_ramp`] and [`close_ramp`].
    open: Vec<u16>,
    close: Vec<u16>,
    /// Which screen this tick belongs to.
    screen: usize,
    /// The tick within that screen.
    tick: u32,
}

impl Cred {
    /// A part with nothing loaded: `init` unpacks the image.
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
        }
    }

    /// One screen's ticks: the parked tick, the show tick, the two loops and
    /// the hold.
    fn screen_ticks(&self) -> u32 {
        SHOW_TICK + 1 + self.open.len() as u32 + HOLD_FRAMES + self.close.len() as u32
    }

    /// `MAIN.C:169-174`: the palette, the caption, then the picture.
    fn show_screen(&self, ctx: &mut PartCtx) {
        let screen = &self.screens[self.screen];
        ctx.set_palette(screen.palette);
        let mut y = TEXT_FIRST_LINE;
        for line in &screen.text {
            self.draw_centred(ctx, TEXT_CENTRE, y, line);
            y += TEXT_LINE_STEP;
        }
        self.draw_picture(ctx, &screen.picture);
    }

    /// `MAIN.C:236-243`, `prtc`: drawn from `x - Σ(fonaw[ch]+2)/2`, the divide
    /// last, so an odd width starts one pixel left of centre.
    fn draw_centred(&self, ctx: &mut PartCtx, x: usize, y: usize, text: &[u8]) {
        let width: usize = text.iter().map(|&ch| usize::from(self.fonaw[usize::from(ch)]) + 2).sum();
        self.draw_line(ctx, x - width / 2, y, text);
    }

    /// `MAIN.C:215-234`, `prt`: `fonaw[ch]` columns from `fonap[ch]`, `FONAY`
    /// rows each, then a two-column gap.
    fn draw_line(&self, ctx: &mut PartCtx, mut x: usize, y: usize, text: &[u8]) {
        for &ch in text {
            let width = usize::from(self.fonaw[usize::from(ch)]);
            let columns = usize::from(self.fonap[usize::from(ch)])..;
            for (column, at) in columns.zip(x..x + width) {
                for row in 0..FONT_ROWS {
                    let value = self.strip[row * FONT_WIDTH + column];
                    put(ctx.surface, at, y + row, value);
                }
            }
            x += width + 2;
        }
    }

    /// `MAIN.C:174`: `for(x=0;x<160;x++) for(y=0;y<100;y++)
    /// tw_putpixel(400+x,400+y*2,pic[y][x]+16)`, in the source's loop order.
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

    /// The view for one step's three registers, or the parked state.
    fn set_display(ctx: &mut PartCtx, display: Display) {
        // `TWEAK.ASM:18-60`: the 640-pixel stride folds in the double scan.
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.y_line_offset = Some(DISPLAY_LINE_STRIDE);
        ctx.view.double_scan = false;
        ctx.view.pan = display.pan;
        ctx.view.set_origin(display.origin);
        // Below the split the address counter restarts at 0, the pan is
        // cleared and the line offset is unchanged (`VGA_ProcessSplit`,
        // `vga_draw.cpp:1139-1158`).
        ctx.view.split = Some(Box::new(Split {
            scanline: split_scanline(display.line_compare),
            view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
        }));
    }

    /// The registers the iris writes at tick `tick` of a screen, or `None`
    /// through the hold. The open loop's first step is the tick after
    /// [`SHOW_TICK`]'s `dis_waitb` (`:168`).
    fn iris_display(&self, tick: u32) -> Option<Display> {
        let step = tick.checked_sub(SHOW_TICK + 1)? as usize;
        if step < self.open.len() {
            return Some(Display::opening(self.open[step]));
        }
        let close = step.checked_sub(self.open.len() + HOLD_FRAMES as usize)?;
        self.close.get(close).map(|y| Display::closing(*y))
    }
}

impl Part for Cred {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:38-39`'s `init()` and `tw_opengraph`, with the font and the 21
    /// screens read out of the shipped image.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let error = |message: String| PartError {
            part: NAME,
            message: format!("{EXE_PATH}: {message}"),
        };
        let file = read_asset(NAME, EXE_PATH)?;
        let image = unpack(&file).map_err(|e| error(e.to_string()))?;
        let data = image_data(&image).map_err(error)?;
        self.strip = data.raster;
        scan_metrics(&self.strip, &data.order, &mut self.fonap, &mut self.fonaw);

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
            let mut palette = picture.palette;
            repair_palette(&mut palette);
            self.screens.push(Screen {
                picture: picture.pixels().to_vec(),
                palette,
                text,
            });
        }

        // `TWEAK.ASM:29-34`: 32,768 word writes with the map mask open, the
        // whole 256 KB.
        ctx.surface.clear(0);
        Cred::set_display(ctx, Display::PARKED);
        Ok(())
    }

    /// One tick of one screen (the module doc's map).
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let tick = self.tick;
        if tick >= self.screen_ticks() {
            return PartStatus::Finished;
        }
        match tick {
            PARKED_TICK => {
                Cred::set_display(ctx, Display::PARKED);
                for pixel in &mut ctx.surface.pixels_mut()[..CLRSCR_PIXELS] {
                    *pixel = 0;
                }
            }
            SHOW_TICK => self.show_screen(ctx),
            _ => {
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
                // `MAIN.C:158-159`: the epilogue is commented out.
                return PartStatus::Finished;
            }
        }
        PartStatus::Continuing
    }
}

/// The pen: `tw_putpixel(x, y, c)` writes plane `x & 3` at byte `y*80 + x/4`
/// (`TWEAK.ASM:87-117`), which is flat pixel `320*y + x`. No write reaches the
/// surface's end (the largest is 191,919).
fn put(surface: &mut Surface, x: usize, y: usize, value: u8) {
    let at = y * LINE_LEN + x;
    if at < surface.pixels().len() {
        surface.pixels_mut()[at] = value;
    }
}

/// `MAIN.C:273-317`, `init`'s repair of each picture's palette:
///
/// 1. `memmove(&pic[16*3+16], &pic[16], 768-16*3)` (`:273-293`): entries 0-239
///    move to 16-255, overlapping, and 0-15 stay.
/// 2. `pic[a*3+k+16] = 7*a` for `a` 0-9 (`:294-317`): the grey ramp in entries
///    0-9, which the caption's strip values index; the picture, drawn at `+16`,
///    shows its own colours.
fn repair_palette(palette: &mut Palette) {
    let source: [[u8; 3]; REPAIR_COUNT] = core::array::from_fn(|i| palette.entry(i as u8));
    for (i, rgb) in source.iter().enumerate() {
        palette.set_entry(i as u8 + REPAIR_SHIFT, *rgb);
    }
    for a in 0..RAMP_ENTRIES {
        palette.set_entry(a, [RAMP_STEP * a, RAMP_STEP * a, RAMP_STEP * a]);
    }
}

/// `MAIN.C:250-269`: each `fonaorder` character takes the next run of inked
/// columns over the first [`FONT_ROWS`] rows, then the blank gets its own
/// entry. A character with no run keeps 0, as the static tables do.
fn scan_metrics(strip: &[u8], order: &[u8], fonap: &mut [u16; 256], fonaw: &mut [u8; 256]) {
    scan_rows(strip, FONT_ROWS, order, fonap, fonaw);
    fonap[usize::from(BLANK)] = BLANK_COLUMN;
    fonaw[usize::from(BLANK)] = BLANK_WIDTH;
}

/// The run scan of [`scan_metrics`] over `rows` rows of the strip.
fn scan_rows(strip: &[u8], rows: usize, order: &[u8], fonap: &mut [u16; 256], fonaw: &mut [u8; 256]) {
    let inked = |x: usize| (0..rows).any(|y| strip[y * FONT_WIDTH + x] != 0);
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::Rig;
    use crate::shell::SURFACE_WIDTH;

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

    /// The width of a caption line, by `prtc`'s arithmetic.
    fn line_width(part: &Cred, line: &[u8]) -> usize {
        line.iter().map(|&ch| usize::from(part.fonaw[usize::from(ch)]) + 2).sum()
    }

    #[test]
    fn the_images_font_and_order_are_where_init_reads_them() {
        // The far array at segment `0x29f`, 40 rows of 1,500 up to the first
        // picture, and the routine's 32-row window (A8); `fonaorder` is
        // `MAIN.C:32`'s 47 characters.
        let image = image();
        let data = image_data(&image).unwrap();
        assert_eq!(&image.image[0x1e83..0x1e86], &[0xba, 0x9f, 0x02], "mov dx,029fh");
        assert_eq!(&image.image[0x1db0..0x1db3], &[0xba, 0x9f, 0x02], "mov dx,029fh in prt");
        assert_eq!(&image.image[0x1e7b..0x1e7e], &[0xba, 0xdc, 0x05], "mov dx,05dch: 1,500");
        assert_eq!(&image.image[0x1e95..0x1e99], &[0x83, 0x7e, 0xfc, 0x20], "FONAY 32");
        assert_eq!(data.raster.len(), FONT_ARRAY_ROWS * FONT_WIDTH);
        assert_eq!(FONAORDER_PTR + 4, DGROUP + 0x96, "the string follows its pointer");
        assert_eq!(data.order, b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789/?!:,.\"()+-");
    }

    #[test]
    fn the_metrics_are_the_runs_the_strip_itself_holds() {
        // One ink run per `fonaorder` character: 447 inked columns, the last
        // ink at column 557.
        let (_, part) = started();
        let order = image_data(&image()).unwrap().order;
        let width = |ch: u8| part.fonaw[usize::from(ch)];
        let column = |ch: u8| part.fonap[usize::from(ch)];
        let mut inked_columns = 0usize;
        for &ch in &order {
            assert!(width(ch) > 0, "{:?} got no run", ch as char);
            let start = usize::from(column(ch));
            assert!((0..FONT_ROWS).any(|y| part.strip[y * FONT_WIDTH + start] != 0));
            inked_columns += usize::from(width(ch));
        }
        assert_eq!(inked_columns, 447, "the strip's inked columns");
        assert_eq!(order.iter().map(|&ch| width(ch)).max(), Some(11), "the widest run");
        assert_eq!(order.iter().map(|&ch| width(ch)).min(), Some(2), "the narrowest run");
        assert_eq!(column(b'-') + u16::from(width(b'-')) - 1, 557, "the last inked column");
        assert_eq!((column(b'A'), width(b'A')), (7, 11));
        assert_eq!(width(b'I'), 2, "the narrow I");
        assert_eq!(width(b'"'), 3, "the quote is the widest of the marks");
        // The routine's own blank, and a character neither scanned nor blank.
        assert_eq!((column(BLANK), width(BLANK)), (1468, 8));
        assert_eq!((column(b'z'), width(b'z')), (0, 0));
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
        let column0 = usize::from(part.fonap[usize::from(b'G')]);
        assert_eq!(column0, 79, "`G` is the strip's seventh run");
        for row in 0..FONT_ROWS {
            for column in 0..usize::from(part.fonaw[usize::from(b'G')]) {
                let expected = part.strip[row * FONT_WIDTH + column0 + column];
                let at = (TEXT_FIRST_LINE + row) * LINE_LEN + start + column;
                assert_eq!(rig.surface.pixels()[at], expected, "({column}, {row})");
            }
        }
        // The last glyph of the line, and the two-column gap after it.
        let last = start + width - 2;
        assert_eq!(
            rig.surface.pixels()[TEXT_FIRST_LINE * LINE_LEN + last - 1],
            part.strip[usize::from(part.fonap[usize::from(b'L')]) + usize::from(part.fonaw[usize::from(b'L')]) - 1],
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
            part.strip[usize::from(part.fonap[usize::from(b'M')])],
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
        // Each call names its picture at `segment * 16` (one blob, 0x419
        // paragraphs, apart from `0x1145`) and its caption at `DGROUP + text`.
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
            // `FC FC`, 160x100, 256 colours, the pixels at paragraph 49.
            let blob = &data.pictures[number];
            assert_eq!(
                &blob[..10],
                &[0xfc, 0xfc, 0xa0, 0x00, 0x64, 0x00, 0x00, 0x01, 0x31, 0x00],
                "screen {screen}'s header"
            );
        }
        // 27 bytes a call: the 12-byte `dis_exit` test and the 15-byte call.
        assert!(data
            .calls
            .windows(2)
            .all(|pair| pair[1].at - pair[0].at == 0x1b));
        let (_, part) = started();
        for (number, screen) in part.screens.iter().enumerate() {
            let decoded = load(&data.pictures[number], Format::Origin).unwrap();
            assert_eq!(screen.picture, decoded.pixels(), "screen {}", number + 1);
            assert_eq!(screen.text, data.captions[number], "screen {}", number + 1);
        }
    }

    #[test]
    fn an_image_without_this_builds_calls_is_refused() {
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
    fn the_picture_is_laid_in_at_the_coordinates_the_source_gives() {
        // Picture pixel (x, y) is flat `(400+2y)*320 + 400 + x`, displayed at
        // logical line `401 + 2y`, column `80 + x`.
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
        // The hold's origin, logical line 401.
        assert_eq!(
            rig.surface.pixels()[128_320 + 80],
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
        // `MAIN.C:294-317`: entries 0-9, the ones the caption's strip values
        // index.
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
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, SHOW_TICK + 1);
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
        // Neither the screen, the display nor the DAC moves through `:194`'s
        // 200 frames, 104..303.
        let (mut rig, mut part) = started();
        let open_last = SHOW_TICK + part.open.len() as u32;
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
        assert_eq!(part.tick, hold_first + HOLD_FRAMES);
    }

    #[test]
    fn the_part_runs_twenty_one_screens_of_the_sources_own_length() {
        // 21 screens, 363 ticks each, then `MAIN.C:158-159`'s `}`.
        let (mut rig, mut part) = started();
        assert_eq!(part.screens.len(), 21, "`MAIN.C:41-156`'s calls, and the image's");
        assert_eq!(part.screen_ticks(), 363);
        let last = part.screen_ticks() * SCREEN_COUNT as u32 - 1;
        assert_eq!(last, 7_622);
        run_ticks(&mut rig, &mut part, last);
        // The last tick draws the last frame and ends `main()`.
        assert_eq!(part.frame(&mut rig.ctx(last)), PartStatus::Finished);
        assert_eq!(part.screen, SCREEN_COUNT);
    }

    #[test]
    fn the_three_registers_move_once_a_tick_through_both_wipes() {
        // The open loop's 102 steps are ticks 2-103 (`MAIN.C:176`), the hold
        // 104-303 (`:194`), the close loop's 59 steps 304-362 (`:196`).
        let (mut rig, mut part) = started();
        let open = part.open.clone();
        let close = part.close.clone();
        assert_eq!((open.len(), open[0], open.last()), (102, 200 * 128, Some(&1)));
        assert_eq!((close.len(), close[0], close.last()), (59, 0, Some(&24_795)));
        // The open loop's last step writes the hold's state.
        assert_eq!(Display::opening(1), Display { origin: 128_320, line_compare: 200, pan: 0 });
        let open_first = SHOW_TICK + 1;
        let hold_first = open_first + open.len() as u32;
        let close_first = hold_first + HOLD_FRAMES;
        assert_eq!((open_first, hold_first, close_first), (2, 104, 304));
        assert_eq!(part.screen_ticks(), close_first + close.len() as u32);

        // (display start, split row, pan in pixels) at named ticks, worked by
        // hand from the two loops, each side of every boundary.
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
                t if t < close_first => Display::opening(1),
                t => Display::closing(close[(t - close_first) as usize]),
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

        // At a wipe step with a pan: above the split the step's start and pan,
        // below it address 0, no pan and the unchanged 640-pixel line offset.
        let (mut rig, mut part) = started();
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
        assert_eq!(rig.view.effective(100).stride(SURFACE_WIDTH as u16), DISPLAY_LINE_STRIDE);
        assert_eq!(rig.view.source_index(&rig.surface, 319, 199), 99 * 640 + 319);
    }

    #[test]
    fn the_eight_rows_the_routine_never_reads_do_not_move_a_run() {
        // `MAIN.C:29` scans 32 rows of a raster that has 40; no column is
        // inked only in rows 32-39, so every run is the same over 40, and
        // the difference is the 92 unread ink pixels, 46 of them on displayed
        // rows (A8).
        let (_, part) = started();
        let strip = part.strip.clone();
        let order = image_data(&image()).unwrap().order;
        let (mut ap32, mut aw32) = ([0u16; 256], [0u8; 256]);
        let (mut ap40, mut aw40) = ([0u16; 256], [0u8; 256]);
        scan_rows(&strip, 32, &order, &mut ap32, &mut aw32);
        scan_rows(&strip, 40, &order, &mut ap40, &mut aw40);
        assert_eq!((ap32, aw32), (ap40, aw40));

        let inked = |rows: std::ops::Range<usize>, x: usize| {
            rows.clone().any(|y| strip[y * FONT_WIDTH + x] != 0)
        };
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
