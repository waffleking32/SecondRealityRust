//! TECHNO — the interference, the bars and the troll.
//!
//! `TECHNO/KOE.C` (728 lines) over `KOEA.ASM` (1,065), `KOEB.ASM` (632),
//! `POLYCLIP.ASM` (273) and `READP.C` (72): **one part, two screens**.
//!
//! 1. **The interference, first pass** (`KOE.C:213-216`): a music gate
//!    (`musplus < -4`), `setmframe(0)`, then `dointerference2` — `KOEB.ASM`'s
//!    `resetmode13`, the circle picture (`init_interference`), and 256 frames
//!    of a palette fade (`do_interference`: `palfader` +2 a frame, the `pal0`
//!    colour mixed into all 16 entries). The screen is the 320×200 EGA
//!    16-colour planar display seen as a 320-pixel window into an 80-byte
//!    plane row (CRTC `13h = 40`, written by `init_interference`'s
//!    `mov ax,2813h`), and the first pass's display start is the constant
//!    8,020 (the `sizefade`-scaled sine is dead: `sizefade` stays 0).
//! 2. **The interference, second pass** (`KOE.C:218-316`): `initinterference`
//!    again, then `KOEA.ASM`'s `do_interference` — 8 rotated copies of the
//!    plane-2 circle copied into plane 3 at a moving 640-wide window, the
//!    `pal1`/`pal2` palette cycle, the `power0`/`sinuspower` ramp, until
//!    `getmframe` reaches 925. Then the five `flash`es, the 16-colour mode
//!    set, the white fill, the four bar wipes (`musrow & 7 == 7` waits) and
//!    the three `doit` routines: 420/840/980 counts of 1-bit boxes clipped by
//!    `POLYCLIP.ASM`, filled by `blit16`/`blit16b`'s prefix-XOR, and shown on
//!    eight (doit1/2) or four (doit3) 8,192-byte pages.
//! 3. **The troll** (`KOE.C:588-727`): mode 13h, `inittwk` (chain-4 off, 400
//!    lines, 160-byte plane rows), `TROLL.UP` decoded by `READP.C`'s RLE and
//!    blitted a plane at a time, the 16 `palfade` tables, `setpalarea`'s
//!    512-byte upload, the order-35 wait, the 300-count pan, the 50-frame
//!    ripple and the final `musplus` wait.
//!
//! # The two screens
//!
//! The 320×200 screen is EGA 16-colour planar: a displayed pixel's index is
//! the four planes' bits at one plane byte's bit, the leftmost pixel being bit
//! 7, at a 40-byte row stride (80 in doit3). The port keeps the four planes as
//! four 64 KiB arrays and composes the window into the engine surface.
//!
//! The 320×400 screen is mode 13h with chain-4 off: the emulator's `M_VGA`
//! draw reads the four-plane interleaved memory as one byte a pixel, so the
//! four planes at one plane byte are four consecutive pixels. The port keeps
//! that interleaved image *in the surface* (640×400 = 4 bytes a plane byte)
//! and lets a `View` read it: the CRTC's plane-byte start ×4 is the view's
//! pixel origin, a 160-byte row is 640 pixels, and the ATC pan is in pixels
//! (`M_VGA` halves the register). The picture starts 80 plane bytes into each
//! row.
//!
//! # The preserved bugs
//!
//! - `_setpalarea` uploads `2*count + start` bytes, not `3*count`: every
//!   `(..., 0, 256)` call is 512 of the table's 768 bytes.
//! - `setborder` returns immediately (`KOE.C:68-80`), and its calls are kept.
//! - `flash(-2)` returns without capturing or uploading.
//! - `drawline`'s `mov cx,400` in `init_interference` is overwritten before it
//!   is read.
//! - `KOEB.ASM`'s plane-8 drawing loop is behind `jmp @@OVER3` and
//!   `sizefade`'s `add ax,256` is commented out, so the first pass's display
//!   start is constant.
//! - `patdir` only ever moves towards smaller offsets; the `>= 24` wrap is
//!   dead.
//! - `rotate1` rotates 31,936 of the 32,000 bytes; the last 64 are untouched.
//! - `pic = halloc(20000, 4)` is 80,000 bytes, and the segment poke is how the
//!   original folded the second 40,000 into the same block.
//! - The `& 2047` sine cursor in `interference_draw` is a **byte** offset
//!   into the 2,048-byte word table, not a word index: the port reads word
//!   `bp/2`. (The MOVE routines keep a word index and `shl bx,1` before
//!   indexing, which is how the two cursors are told apart.) This is not
//!   the alias tail the first reading assumed.
//! - `clipanypoly` clips the closing edge before the loop and `cap3`'s
//!   `jmp cap35` makes its result the first edge, skipping the first
//!   iteration's clipping; the first edge is not a pseudo-point.

use std::path::Path;

use sr_assets::{load_path, Image};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

const NAME: &str = "techno";

// ---------------------------------------------------------------- assets --
/// `_circle2`: a raw EGA planar picture, 200 rows of 40 bytes a plane.
const CIRCLE2_PATH: &str = "assets/TECHNO/CIRCLE2.EGA";
/// `_circle`: `DDSTARS/PIC.EGA`, 200 rows of three interleaved 40-byte planes.
const CIRCLE_PATH: &str = "assets/DDSTARS/PIC.EGA";
/// `TROLL.UP`: the Origin RLE picture, 320×400, 256 colours.
const TROLL_PATH: &str = "assets/TECHNO/TROLL.UP";

// ---------------------------------------------------------------- screens --
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
const TROLL_HEIGHT: u16 = 400;
/// One VGA plane's addressable bytes; every plane offset wraps here.
const PLANE_BYTES: usize = 65_536;
/// 40 plane bytes a row: the bars screen (`mode16`'s `13h+256*20`, 20 words).
const PLANE_ROW: usize = 40;
/// 80 plane bytes a row: the interference and doit3 (`init_interference`'s
/// `mov ax,2813h`; doit3's `13h+256*40`, 40 words).
const PLANE_ROW_WIDE: usize = 80;
/// 160 plane bytes a row: the troll (`13h+256*80`, 80 words).
const TROLL_ROW: usize = 160;
/// One 320×200 page in plane bytes.
const PAGE_BYTES: usize = 8_000;
/// `vram = 0xa0000000 + 0x2000000*plv`: 0x200 paragraphs = 8,192 plane bytes.
const PAGE_STEP: usize = 0x2000;
/// `CRTC 0Ch = plv*0x20`: the start-address high byte for a page.
const PAGE_HIGH: i32 = 0x20;
/// `vbuf = halloc(8192, 1)`, of which 8,000 are used.
const VBUF_BYTES: usize = 8_000;

// ---------------------------------------------------------------- tables --
/// `KOE.C:119-168`'s 16 colour blocks, 16 entries each, six bits a component.
const BAR_COLOURS: usize = 16;
/// `KOEB.ASM`'s `pal0`: the first pass's colour, `(0,30,40)` at the two
/// positions the 8-triple window can land on.
const PAL0: [u8; 48] = [
    0, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 40, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// `KOEA.ASM`'s `pal1`: `(v, v*8/9, v)`.
const PAL1: [u8; 48] = [
    30, 26, 30, 60, 53, 60, 50, 44, 50, 40, 35, 40, 30, 26, 30, 20, 17, 20, 10, 8, 10, 0, 0, 0,
    30, 26, 30, 60, 53, 60, 50, 44, 50, 40, 35, 40, 30, 26, 30, 20, 17, 20, 10, 8, 10, 0, 0, 0,
];
/// `KOEA.ASM`'s `pal2`: `(v, v*7/9, v)`.
const PAL2: [u8; 48] = [
    0, 0, 0, 10, 7, 10, 20, 15, 20, 30, 23, 30, 40, 31, 40, 50, 38, 50, 60, 46, 60, 30, 23, 30,
    0, 0, 0, 10, 7, 10, 20, 15, 20, 30, 23, 30, 40, 31, 40, 50, 38, 50, 60, 46, 60, 30, 23, 30,
];

/// The byte-reversal `flip8` table (`KOEA.ASM:425-438`), built from its rule
/// rather than transcribed.
fn flip8_table() -> [u8; 256] {
    let mut table = [0u8; 256];
    for (value, entry) in table.iter_mut().enumerate() {
        *entry = (value as u8).reverse_bits();
    }
    table
}

/// `blitinit`'s `_blit16t` (`KOEA.ASM:84-114`), 256 two-byte entries.
///
/// The table turns the vbuf's 1-bit-per-pixel outline into the plane byte the
/// display fills: the low byte is the source byte's suffix parity, so the
/// written row is the row's prefix XOR — the XOR-filled polygon. The high byte
/// is the parity `(low & 1) << 7` the chain carries to the next byte.
fn blit_table() -> [(u8, u8); 256] {
    let mut table = [(0u8, 0u8); 256];
    for (value, entry) in table.iter_mut().enumerate() {
        let mut dh = 255u8;
        let mut dl = value as u8;
        let mut ah = 0u8;
        let mut carry = 0u8;
        for _ in 0..8 {
            let next = dl >> 7;
            dl = (dl << 1) | carry;
            carry = next;
            if carry != 0 {
                ah ^= dh;
            }
            dh >>= 1;
        }
        *entry = (ah, ((ah >> 1) | ((ah & 1) << 7)) & 0x80);
    }
    table
}

/// `KOE.C:173-211`: `power0`/`power1`, 16 rows of 256 signed bytes. Row `b`
/// scales the signed byte by `b/15`; row 15 is the byte itself.
fn power_table() -> [i8; 4096] {
    let mut table = [0i8; 4096];
    for b in 0..16usize {
        for c in 0..256usize {
            let a = if c > 127 { c as i32 - 256 } else { c as i32 };
            table[b * 256 + c] = if b == 15 { c as u8 as i8 } else { (a * b as i32 / 15) as i8 };
        }
    }
    table
}

/// The bar palette (`KOE.C:119-168`): 16 blocks of 16 entries. Entry `a`'s
/// base colour is chosen by `a`'s popcount and scaled per block by
/// `(10 + c*9/9)/10`, `(10 + c*7/9)/10` and `(10 + c*5/9)/10`, clamped at 63.
fn bar_palette() -> [u8; 768] {
    const BASE: [[i32; 3]; 5] = [
        [0, 0, 0],
        [38 * 64 / 111, 33 * 64 / 111, 44 * 64 / 111],
        [52 * 64 / 111, 45 * 64 / 111, 58 * 64 / 111],
        [67 * 64 / 111, 61 * 64 / 111, 73 * 64 / 111],
        [83 * 64 / 111, 77 * 64 / 111, 89 * 64 / 111],
    ];
    let mut pal = [0u8; BAR_COLOURS * 16 * 3];
    for c in 0..BAR_COLOURS as i32 {
        for a in 0..16usize {
            let x = a.count_ones() as usize;
            let [r, g, b] = BASE[x];
            let at = (c as usize * 16 + a) * 3;
            pal[at] = (r * (10 + c * 9 / 9) / 10).min(63) as u8;
            pal[at + 1] = (g * (10 + c * 7 / 9) / 10).min(63) as u8;
            pal[at + 2] = (b * (10 + c * 5 / 9) / 10).min(63) as u8;
        }
    }
    pal
}

// ------------------------------------------------------------- the tables --
/// `SIN1024.INC`: 1,024 signed words, a full sine scaled to 256 (the
/// table's max is 256). `KOEA.ASM` keeps two cursors into it: the MOVE keeps
/// a word index (`and bx,1023; shl bx,1` before `_sin1024[bx]`), while
/// `interference_draw` keeps a **byte** offset (`add bp,9*2; and bp,2047`)
/// and therefore reads word `bp/2`. The unit test rebuilds all 1,024 words
/// from the tree.
const SIN_TABLE: [i16; 1024] = [
    0, 1, 3, 4, 6, 7, 9, 10, 12, 14, 15, 17, 18, 20, 21, 23,
    25, 26, 28, 29, 31, 32, 34, 36, 37, 39, 40, 42, 43, 45, 46, 48,
    49, 51, 53, 54, 56, 57, 59, 60, 62, 63, 65, 66, 68, 69, 71, 72,
    74, 75, 77, 78, 80, 81, 83, 84, 86, 87, 89, 90, 92, 93, 95, 96,
    97, 99, 100, 102, 103, 105, 106, 108, 109, 110, 112, 113, 115, 116, 117, 119,
    120, 122, 123, 124, 126, 127, 128, 130, 131, 132, 134, 135, 136, 138, 139, 140,
    142, 143, 144, 146, 147, 148, 149, 151, 152, 153, 155, 156, 157, 158, 159, 161,
    162, 163, 164, 166, 167, 168, 169, 170, 171, 173, 174, 175, 176, 177, 178, 179,
    181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196,
    197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 211,
    212, 213, 214, 215, 216, 217, 217, 218, 219, 220, 221, 221, 222, 223, 224, 225,
    225, 226, 227, 227, 228, 229, 230, 230, 231, 232, 232, 233, 234, 234, 235, 235,
    236, 237, 237, 238, 238, 239, 239, 240, 241, 241, 242, 242, 243, 243, 244, 244,
    244, 245, 245, 246, 246, 247, 247, 247, 248, 248, 249, 249, 249, 250, 250, 250,
    251, 251, 251, 251, 252, 252, 252, 252, 253, 253, 253, 253, 254, 254, 254, 254,
    254, 254, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    256, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254,
    254, 254, 254, 254, 254, 253, 253, 253, 253, 252, 252, 252, 252, 251, 251, 251,
    251, 250, 250, 250, 249, 249, 249, 248, 248, 247, 247, 247, 246, 246, 245, 245,
    244, 244, 244, 243, 243, 242, 242, 241, 241, 240, 239, 239, 238, 238, 237, 237,
    236, 235, 235, 234, 234, 233, 232, 232, 231, 230, 230, 229, 228, 227, 227, 226,
    225, 225, 224, 223, 222, 221, 221, 220, 219, 218, 217, 217, 216, 215, 214, 213,
    212, 211, 211, 210, 209, 208, 207, 206, 205, 204, 203, 202, 201, 200, 199, 198,
    197, 196, 195, 194, 193, 192, 191, 190, 189, 188, 187, 186, 185, 184, 183, 182,
    181, 179, 178, 177, 176, 175, 174, 173, 171, 170, 169, 168, 167, 166, 164, 163,
    162, 161, 159, 158, 157, 156, 155, 153, 152, 151, 149, 148, 147, 146, 144, 143,
    142, 140, 139, 138, 136, 135, 134, 132, 131, 130, 128, 127, 126, 124, 123, 122,
    120, 119, 117, 116, 115, 113, 112, 110, 109, 108, 106, 105, 103, 102, 100, 99,
    97, 96, 95, 93, 92, 90, 89, 87, 86, 84, 83, 81, 80, 78, 77, 75,
    74, 72, 71, 69, 68, 66, 65, 63, 62, 60, 59, 57, 56, 54, 53, 51,
    49, 48, 46, 45, 43, 42, 40, 39, 37, 36, 34, 32, 31, 29, 28, 26,
    25, 23, 21, 20, 18, 17, 15, 14, 12, 10, 9, 7, 6, 4, 3, 1,
    0, -1, -3, -4, -6, -7, -9, -10, -12, -14, -15, -17, -18, -20, -21, -23,
    -25, -26, -28, -29, -31, -32, -34, -36, -37, -39, -40, -42, -43, -45, -46, -48,
    -49, -51, -53, -54, -56, -57, -59, -60, -62, -63, -65, -66, -68, -69, -71, -72,
    -74, -75, -77, -78, -80, -81, -83, -84, -86, -87, -89, -90, -92, -93, -95, -96,
    -97, -99, -100, -102, -103, -105, -106, -108, -109, -110, -112, -113, -115, -116, -117, -119,
    -120, -122, -123, -124, -126, -127, -128, -130, -131, -132, -134, -135, -136, -138, -139, -140,
    -142, -143, -144, -146, -147, -148, -149, -151, -152, -153, -155, -156, -157, -158, -159, -161,
    -162, -163, -164, -166, -167, -168, -169, -170, -171, -173, -174, -175, -176, -177, -178, -179,
    -181, -182, -183, -184, -185, -186, -187, -188, -189, -190, -191, -192, -193, -194, -195, -196,
    -197, -198, -199, -200, -201, -202, -203, -204, -205, -206, -207, -208, -209, -210, -211, -211,
    -212, -213, -214, -215, -216, -217, -217, -218, -219, -220, -221, -221, -222, -223, -224, -225,
    -225, -226, -227, -227, -228, -229, -230, -230, -231, -232, -232, -233, -234, -234, -235, -235,
    -236, -237, -237, -238, -238, -239, -239, -240, -241, -241, -242, -242, -243, -243, -244, -244,
    -244, -245, -245, -246, -246, -247, -247, -247, -248, -248, -249, -249, -249, -250, -250, -250,
    -251, -251, -251, -251, -252, -252, -252, -252, -253, -253, -253, -253, -254, -254, -254, -254,
    -254, -254, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255,
    -256, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -254,
    -254, -254, -254, -254, -254, -253, -253, -253, -253, -252, -252, -252, -252, -251, -251, -251,
    -251, -250, -250, -250, -249, -249, -249, -248, -248, -247, -247, -247, -246, -246, -245, -245,
    -244, -244, -244, -243, -243, -242, -242, -241, -241, -240, -239, -239, -238, -238, -237, -237,
    -236, -235, -235, -234, -234, -233, -232, -232, -231, -230, -230, -229, -228, -227, -227, -226,
    -225, -225, -224, -223, -222, -221, -221, -220, -219, -218, -217, -217, -216, -215, -214, -213,
    -212, -211, -211, -210, -209, -208, -207, -206, -205, -204, -203, -202, -201, -200, -199, -198,
    -197, -196, -195, -194, -193, -192, -191, -190, -189, -188, -187, -186, -185, -184, -183, -182,
    -181, -179, -178, -177, -176, -175, -174, -173, -171, -170, -169, -168, -167, -166, -164, -163,
    -162, -161, -159, -158, -157, -156, -155, -153, -152, -151, -149, -148, -147, -146, -144, -143,
    -142, -140, -139, -138, -136, -135, -134, -132, -131, -130, -128, -127, -126, -124, -123, -122,
    -120, -119, -117, -116, -115, -113, -112, -110, -109, -108, -106, -105, -103, -102, -100, -99,
    -97, -96, -95, -93, -92, -90, -89, -87, -86, -84, -83, -81, -80, -78, -77, -75,
    -74, -72, -71, -69, -68, -66, -65, -63, -62, -60, -59, -57, -56, -54, -53, -51,
    -49, -48, -46, -45, -43, -42, -40, -39, -37, -36, -34, -32, -31, -29, -28, -26,
    -25, -23, -21, -20, -18, -17, -15, -14, -12, -10, -9, -7, -6, -4, -3, -1,
];

// ---------------------------------------------------------------- the part --

/// The entry pre-roll: polls that put the published position at MUSIC1 order
/// 18 row 60 — the trace's `second2` reading at 4797 — with `musplus` -4, so
/// the first gate (`<-4`) passes on tick 0. A part reached mid-chain does not
/// poll (`music_fresh`); its player is already past this stretch.
const MUSIC_PRE_ROLL: u32 = 253;
/// `KOE.C:213`: `while(!dis_exit() && dis_musplus()<-4);`
const GATE_ONE: i32 = -4;
/// `KOE.C:275`: `while(!dis_exit() && dis_musplus()<-3);`
const GATE_TWO: i32 = -3;
/// `KOEA.ASM:900-904`: the second pass ends at music frame 925.
const INTERFERENCE_END: i32 = 925;
/// `KOE.C:328-330`: `doit1(70*6)`, `doit2(70*12)`, `doit3(70*14)`.
const DOIT1_COUNT: i32 = 70 * 6;
const DOIT2_COUNT: i32 = 70 * 12;
const DOIT3_COUNT: i32 = 70 * 14;
/// `KOE.C:496`: doit3's `getmframe > 2520` break.
const DOIT3_MFRAME: i32 = 2520;
/// `KOE.C:501`: the countdown begins below 333.
const DOIT3_COUNTDOWN: i32 = 333;

/// Where the part is in the source's straight line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Gate,
    /// `resetmode13`'s first waitb has returned: clear the DAC.
    ResetClear,
    /// The second waitb has returned: the mode set, the picture, the loop's
    /// first wait.
    ResetMode,
    /// `KOEB.ASM`'s `do_interference`, 256 iterations.
    FirstPass,
    /// `KOEA.ASM`'s `do_interference`, until music frame 925.
    SecondPass,
    Flash32,
    Flash64,
    Flash192,
    Flash256,
    /// `while(musplus < -3)`.
    MusplusMinus3,
    /// `while((musrow & 7) != 7)` before the first bar.
    RowWait,
    /// One of the four bar wipes.
    Wipe,
    /// The wipe's `musrow & 7 == 7` wait.
    WipeRowWait,
    /// The wipe's five flashes after the wait.
    WipeFlash,
    Doit1,
    Doit2,
    Doit3Prologue,
    Doit3,
    /// The mode 13h / inittwk transition and the first of the troll's waits.
    TrollWait1,
    TrollWait2,
    TrollBuild,
    TrollPalette,
    TrollOrderWait,
    TrollPan,
    TrollRipple,
    TrollFinalWait,
    Done,
}

pub struct Techno {
    /// The directory the assets are read from: empty for a run (the shell
    /// starts in the repository root), `"../../"` for the unit tests.
    root: &'static str,
    phase: Phase,
    tick: u32,
    /// The three polls the emulator's CPU-bound preamble takes between the
    /// `setmframe(0)` and the first pass's first iteration: six rendered
    /// frames (4794..4800) over this part's three preamble ticks.
    extra_polls: u8,
    // ---- assets ----
    circle2: Vec<u8>,
    circle: Vec<u8>,
    troll_pixels: Vec<u8>,
    troll_palette: [u8; 768],
    // ---- tables ----
    bar_palette: [u8; 768],
    power0: Vec<i8>,
    /// `KOE.C:173-211` computes `power1` and the first pass carries
    /// `overx`/`overya`, but `KOEB.ASM` never reads them; kept as the
    /// source's dead statics rather than dropped from the transliteration.
    #[allow(dead_code)]
    power1: Vec<i8>,
    flip8: [u8; 256],
    blit: Vec<(u8, u8)>,
    // ---- the video memory and the DAC ----
    planes: [Vec<u8>; 4],
    circles: Vec<Vec<u8>>,
    dac: [u8; 768],
    /// CRTC start address in plane bytes (high byte << 8 | low byte).
    display_start: i32,
    /// Plane bytes a displayed row advances.
    stride: usize,
    /// ATC 13h, in displayed pixels (the two modes interpret it differently).
    pan: u8,
    // ---- the first pass (`KOEB.ASM`'s own statics) ----
    palanimc: i32,
    patdir: i32,
    palfader: i32,
    mix: [u8; 48],
    p1_framecount: i32,
    p1_rotspeed: i32,
    p1_sizefade: i32,
    p1_scrnrot: i32,
    p1_scrnx: i32,
    p1_scrny: i32,
    p1_scrnpos: Option<i32>,
    p1_scrnposl: i32,
    p1_overrot: i32,
    /// `KOEB.ASM`'s `overx`/`overya`: assigned and never read (the same
    /// statics the source itself leaves dead). Kept for the transliteration.
    #[allow(dead_code)]
    p1_overx: i32,
    #[allow(dead_code)]
    p1_overya: i32,
    // ---- the second pass (`KOEA.ASM`'s own statics) ----
    p2_palanimc: i32,
    p2_pal2: i32,
    p2_patdir: i32,
    p2_framecount: i32,
    p2_sinurot: i32,
    p2_overrot: i32,
    p2_overx: i32,
    p2_overya: i32,
    p2_scrnrot: i32,
    p2_scrnx: i32,
    p2_scrny: i32,
    p2_scrnpos: i32,
    p2_scrnposl: i32,
    p2_sinuspower: u8,
    p2_powercnt: u8,
    /// The second pass's first drawn frame is displayed through the previous
    /// frame's start register: the reference's 5058 shows the overlay drawn at
    /// 0 through the first pass's 8,020 stream. Later frames show the MOVE of
    /// the iteration before.
    p2_first_frame: bool,
    // ---- the flashes ----
    flash_pal: [i16; 48],
    // ---- waitborder ----
    last_row: i32,
    curpal: i32,
    // ---- the bar wipes ----
    wipe_band: usize,
    wipe_step: i32,
    wipe_zy: i32,
    wipe_zya: i32,
    wipe_flash: u8,
    wipe_pending: Option<i32>,
    // ---- doit1/2/3 ----
    doit_count: i32,
    doit_first: bool,
    rot: i32,
    rota: i32,
    rot2: i32,
    vm: i32,
    vma: i32,
    plv: i32,
    pl: i32,
    page: usize,
    vbuf: Vec<u8>,
    xpos: i32,
    xposa: i32,
    doit_wx: i32,
    doit_wy: i32,
    repeat: i32,
    ripple: i32,
    ripplep: i32,
    ripple_c: i32,
    // ---- the polygon clipper's globals (`POLYCLIP.ASM`) ----
    clip_x1: i16,
    clip_y1: i16,
    clip_x2: i16,
    clip_y2: i16,
    clipxy2: [(i16, i16); 32],
    polyxy: [(i16, i16); 32],
    polyisides: usize,
    polysides: usize,
    // ---- the troll ----
    palfade: Vec<u8>,
}

impl Techno {
    /// A part with nothing loaded: `init` reads the assets and builds the
    /// tables.
    pub fn new() -> Techno {
        Techno {
            root: "",
            phase: Phase::Gate,
            tick: 0,
            extra_polls: 0,
            circle2: Vec::new(),
            circle: Vec::new(),
            troll_pixels: Vec::new(),
            troll_palette: [0; 768],
            bar_palette: bar_palette(),
            power0: power_table().to_vec(),
            power1: power_table().to_vec(),
            flip8: flip8_table(),
            blit: blit_table().to_vec(),
            planes: std::array::from_fn(|_| vec![0u8; PLANE_BYTES]),
            circles: (0..8).map(|_| vec![0u8; 32_768]).collect(),
            dac: [0; 768],
            display_start: 0,
            stride: PLANE_ROW_WIDE,
            pan: 0,
            palanimc: 0,
            patdir: -3,
            palfader: 0,
            mix: [0; 48],
            p1_framecount: 0,
            p1_rotspeed: 0,
            p1_sizefade: 0,
            p1_scrnrot: 0,
            p1_scrnx: 160,
            p1_scrny: 100,
            p1_scrnpos: Some(0),
            p1_scrnposl: 0,
            p1_overrot: 0,
            p1_overx: 0,
            p1_overya: 0,
            p2_palanimc: 0,
            p2_pal2: 0,
            p2_patdir: 0,
            p2_framecount: 0,
            p2_sinurot: 0,
            p2_overrot: 211,
            p2_overx: 0,
            p2_overya: 0,
            p2_scrnrot: 0,
            p2_scrnx: 0,
            p2_scrny: 0,
            p2_scrnpos: 0,
            p2_scrnposl: 0,
            p2_sinuspower: 0,
            p2_powercnt: 0,
            p2_first_frame: false,
            flash_pal: [0; 48],
            last_row: 0,
            curpal: 0,
            wipe_band: 0,
            wipe_step: 0,
            wipe_zy: 0,
            wipe_zya: 0,
            wipe_flash: 0,
            wipe_pending: None,
            doit_count: 0,
            doit_first: true,
            rot: 45,
            rota: 10,
            rot2: 0,
            vm: 0,
            vma: 0,
            plv: 0,
            pl: 1,
            page: 0,
            vbuf: vec![0u8; VBUF_BYTES],
            xpos: 320,
            xposa: 0,
            doit_wx: 160,
            doit_wy: 100,
            repeat: 1,
            ripple: 0,
            ripplep: 8,
            ripple_c: 0,
            clip_x1: 0,
            clip_y1: 0,
            clip_x2: 0,
            clip_y2: 0,
            clipxy2: [(0, 0); 32],
            polyxy: [(0, 0); 32],
            polyisides: 4,
            polysides: 0,
            palfade: Vec::new(),
        }
    }

    /// `_setpalarea`'s byte count: `2*count + start`, at byte offset `start`.
    fn set_pal_area(&mut self, src: &[u8], start: usize, count: usize) {
        let bytes = 2 * count + start;
        let end = (start + bytes).min(768);
        let take = end.saturating_sub(start);
        self.dac[start..end].copy_from_slice(&src[..take]);
    }

    /// `outpal`: `count` bytes from `src` to the DAC at byte `start`.
    fn upload_dac(&mut self, start: usize, src: &[u8]) {
        let n = src.len().min(768 - start);
        self.dac[start..start + n].copy_from_slice(&src[..n]);
    }

    /// Upload one 16-entry bar block to DAC entries 0..15.
    fn upload_bar(&mut self) {
        let at = (self.curpal as usize) * 48;
        let block = self.bar_palette[at..at + 48].to_vec();
        self.upload_dac(0, &block);
        if self.curpal > 0 {
            self.curpal -= 1;
        }
    }

    /// `waitborder`'s read half: the row change that arms the 15-block slide.
    fn bar_read(&mut self, row: i32) {
        if row != self.last_row {
            self.last_row = row;
            if row & 7 == 7 {
                self.curpal = 15;
            }
        }
    }

    /// `flash(-1)`: read the 16 DAC entries into `pal1`.
    fn flash_capture(&mut self) {
        for (i, value) in self.flash_pal.iter_mut().enumerate() {
            *value = i16::from(self.dac[i]);
        }
    }

    /// `flash(i)`: `(pal1*j + 63*i) >> 8`, `j = 256 - i`, to DAC 0..15.
    fn flash_apply(&mut self, i: i32) {
        if i == -2 {
            return;
        }
        let j = 256 - i;
        let mut bytes = [0u8; 48];
        for (a, out) in bytes.iter_mut().enumerate() {
            let value = (self.flash_pal[a] as i32 * j + 63 * i) >> 8;
            *out = (value & 0x3F) as u8;
        }
        self.upload_dac(0, &bytes);
    }

    /// Write `data` to `mask`'s planes at a plane offset, one plane after
    /// another (`bltline`): each plane reads the same 40 source bytes.
    fn bltline(&mut self, mut mask: u8, planes: u32, src: &[u8], dst: usize) {
        for _ in 0..planes {
            let plane = (mask.trailing_zeros() as usize).min(3);
            for (k, value) in src.iter().enumerate().take(40) {
                self.planes[plane][(dst + k) & (PLANE_BYTES - 1)] = *value;
            }
            mask <<= 1;
        }
    }

    /// `bltlinerev`: the same planes, the 40 bytes reversed and bit-reversed.
    fn bltlinerev(&mut self, mut mask: u8, planes: u32, src: &[u8], dst: usize) {
        for _ in 0..planes {
            let plane = (mask.trailing_zeros() as usize).min(3);
            for k in 0..40usize {
                self.planes[plane][(dst + k) & (PLANE_BYTES - 1)] = self.flip8[src[39 - k] as usize];
            }
            mask <<= 1;
        }
    }

    /// `KOEA.ASM`/`KOEB.ASM` `init_interference`: the two circle pictures,
    /// top and bottom (vertically mirrored), forward and mirrored, then the
    /// eight rotated plane-2 copies.
    fn init_interference(&mut self) {
        self.stride = PLANE_ROW_WIDE; // CRTC 13h = 28h
        for y in 0..200usize {
            let row: [u8; 40] = self.circle2[y * 40..(y + 1) * 40].try_into().unwrap();
            for base in [y * 80, (399 - y) * 80] {
                self.bltline(4, 1, &row, base);
                self.bltlinerev(4, 1, &row, base + 40);
            }
        }
        self.circles[0][..32_000].copy_from_slice(&self.planes[2][..32_000]);
        for i in 0..7usize {
            let (done, rest) = self.circles.split_at_mut(i + 1);
            rotate1(&done[i], &mut rest[0]);
        }
        for y in 0..200usize {
            let row: [u8; 120] = self.circle[y * 120..(y + 1) * 120].try_into().unwrap();
            for base in [y * 80, (399 - y) * 80] {
                for plane in 0..3usize {
                    let src = &row[plane * 40..plane * 40 + 40];
                    self.bltline(1 << plane, 1, src, base);
                    self.bltlinerev(1 << plane, 1, src, base + 40);
                }
            }
        }
        self.p2_framecount = 0;
    }

    /// `KOEB.ASM`'s `do_interference`, one iteration: the palette fade, then
    /// the display start (`sizefade` is 0, so it is the constant 8,020).
    fn first_pass_iteration(&mut self, musrow: i32) {
        // The ATC pan is scrnposl, which the previous iteration left.
        self.pan = self.p1_scrnposl as u8;
        self.palanimc += self.patdir;
        if self.palanimc < 0 {
            self.palanimc = 8 * 3 - 3;
        }
        if self.palanimc >= 8 * 3 {
            self.palanimc = 0;
        }
        self.palfader = (self.palfader + 2).min(512);
        for half in 0..2usize {
            for j in 0..8usize {
                let at = self.palanimc as usize + j * 3;
                let src = [PAL0[at], PAL0[at + 1], PAL0[at + 2]];
                let d = self.palfader;
                let out = if d <= 256 {
                    [
                        ((src[0] as i32 * d) >> 8) as u8,
                        ((src[1] as i32 * d) >> 8) as u8,
                        ((src[2] as i32 * d) >> 8) as u8,
                    ]
                } else {
                    let add = d - 256;
                    [
                        (src[0] as i32 + add).min(63) as u8,
                        (src[1] as i32 + add).min(63) as u8,
                        (src[2] as i32 + add).min(63) as u8,
                    ]
                };
                let at = half * 24 + j * 3;
                self.mix[at..at + 3].copy_from_slice(&out);
            }
        }
        let mix = self.mix;
        self.upload_dac(0, &mix);
        // The MOVE section: the row change, then the (dead-path) sine walk.
        if musrow & 7 == 0 || musrow & 7 == 4 {
            self.patdir = -3;
        }
        self.first_pass_move();
        self.p1_framecount += 1;
    }

    /// `KOEB.ASM:502-590`'s MOVE, with the dead `sizefade` multiply kept.
    fn first_pass_move(&mut self) {
        self.p1_scrnrot = (self.p1_scrnrot + 5) & 1023;
        let sin = SIN_TABLE[self.p1_scrnrot as usize] as i32;
        let scrnx = ((sin.wrapping_mul(self.p1_sizefade) >> 16) + 160) as i16 as i32;
        let sin = SIN_TABLE[((self.p1_scrnrot + 256) & 1023) as usize] as i32;
        let scrny = ((sin.wrapping_mul(self.p1_sizefade) >> 16) + 100) as i16 as i32;
        self.p1_scrnx = scrnx;
        self.p1_scrny = scrny;
        self.p1_overrot = (self.p1_overrot + self.p1_rotspeed) & 1023;
        self.p1_scrnposl = scrnx & 7;
        self.p1_scrnpos = Some(80 * scrny + (scrnx >> 3));
        self.display_start = self.p1_scrnpos.unwrap();
    }

    /// `KOEA.ASM`'s `do_interference`, one iteration.
    fn second_pass_iteration(&mut self, musrow: i32) {
        // The displayed frame is the previous iteration's MOVE: its start
        // register and pan are written mid-frame, after the picture the
        // emulator shows was drawn. The first frame is the exception the
        // reference pins: the overlay is drawn at the static's 0 while the
        // screen still shows the first pass's 8,020.
        if self.p2_first_frame {
            self.p2_first_frame = false;
        } else {
            self.display_start = self.p2_scrnpos;
        }
        self.pan = self.p2_scrnposl as u8;
        // The two palette windows.
        let at = self.p2_palanimc as usize;
        let mut bytes = [0u8; 48];
        bytes[..24].copy_from_slice(&PAL1[at..at + 24]);
        bytes[24..].copy_from_slice(&PAL2[at..at + 24]);
        self.upload_dac(0, &bytes);
        self.interference_draw();
        // The MOVE section.
        let r = musrow & 7;
        if r == 0 || r == 4 {
            self.p2_patdir = -3;
        }
        self.p2_scrnrot = (self.p2_scrnrot + 5) & 1023;
        let scrnx = ((SIN_TABLE[self.p2_scrnrot as usize] as i32) >> 2) + 160;
        let scrny = ((SIN_TABLE[((self.p2_scrnrot + 256) & 1023) as usize] as i32) >> 2) + 100;
        self.p2_scrnx = scrnx;
        self.p2_scrny = scrny;
        self.p2_overrot = (self.p2_overrot + 7) & 1023;
        self.p2_overx = ((SIN_TABLE[self.p2_overrot as usize] as i32) >> 2) + 160;
        let oy = ((SIN_TABLE[((self.p2_overrot + 256) & 1023) as usize] as i32) >> 2) + 100;
        self.p2_overya = (oy * 80) & 0xFFFF;
        self.p2_scrnposl = scrnx & 7;
        self.p2_scrnpos = 80 * scrny + (scrnx >> 3);
        // The source tests `framecount` against 350 *before* incrementing it
        // (`KOEA.ASM:887-897`), so the ramp starts one frame later than a
        // post-increment check would put it.
        if self.p2_framecount >= 70 * 5 {
            self.p2_powercnt = self.p2_powercnt.wrapping_add(1);
            if self.p2_powercnt >= 16 {
                self.p2_powercnt = 0;
                if self.p2_sinuspower < 15 {
                    self.p2_sinuspower += 1;
                }
            }
        }
        self.p2_framecount += 1;
        // The `& 2047` sine walk and the palette cursor.
        self.p2_palanimc += self.p2_patdir;
        if self.p2_palanimc < 0 {
            self.p2_palanimc = 8 * 3 - 3;
        }
        if self.p2_palanimc >= 8 * 3 {
            self.p2_palanimc = 0;
        }
        self.p2_pal2 = self.p2_palanimc;
    }

    /// The moving circles in plane 3, 44 bytes a row at a 640-wide address
    /// space (`KOEA.ASM:779-818`).
    fn interference_draw(&mut self) {
        self.p2_sinurot = (self.p2_sinurot + 14) & 2047;
        let mut bp = self.p2_sinurot;
        for row in 0..200usize {
            bp = (bp + 18) & 2047;
            // `bp` is a byte offset into the word table (`and bp,2047` is
            // 1,024 words), so the sine's index is half of it.
            let mut bx = (SIN_TABLE[(bp >> 1) as usize] as i32) >> 3;
            bx = (bx & 0xFF) | (i32::from(self.p2_sinuspower) << 8);
            let a = self.power0[(bx & 0xFFFF) as usize] as i32;
            let ax = a - self.p2_scrnposl + self.p2_overx;
            let which = (7 - (ax & 7)) as usize;
            // The source's `push si` / `pop si` restores the row's base and
            // adds 80 to it each iteration (`KOEA.ASM:790,812-814`), so row `r`
            // reads from `80*r + overya + (ax >> 3)`.
            let si = (row as i32 * 80 + self.p2_overya + (ax >> 3)) as u16 as usize;
            let di = self.p2_scrnpos as usize + row * 80;
            let circle = &self.circles[which];
            for k in 0..44usize {
                self.planes[3][(di + k) & (PLANE_BYTES - 1)] = circle[si + k];
            }
        }
    }

    /// The 320×200 window: four plane bits a pixel at a bit position.
    fn compose_ega(&self, surface: &mut [u8]) {
        let start = self.display_start as usize;
        let stride = self.stride;
        let pan = usize::from(self.pan);
        for y in 0..usize::from(SCREEN_HEIGHT) {
            let base = (start + y * stride) & (PLANE_BYTES - 1);
            let row = &mut surface[y * 640..y * 640 + 320];
            for (bit, pixel) in (pan..).zip(row.iter_mut()) {
                let at = (base + (bit >> 3)) & (PLANE_BYTES - 1);
                let shift = 7 - (bit & 7);
                *pixel = ((self.planes[0][at] >> shift) & 1)
                    | (((self.planes[1][at] >> shift) & 1) << 1)
                    | (((self.planes[2][at] >> shift) & 1) << 2)
                    | (((self.planes[3][at] >> shift) & 1) << 3);
            }
        }
    }

    /// `KOEA.ASM`'s `_lineblit`: the 320 pixels of `row` become 40 plane
    /// bytes a plane, pixel `x` in plane `x & 3` at byte `x >> 2`.
    fn lineblit(&mut self, row: &[u8], y: usize) {
        for plane in 0..4usize {
            for byte in 0..80usize {
                let at = 80 + y * TROLL_ROW + byte;
                self.planes[plane][at & (PLANE_BYTES - 1)] = row[4 * byte + plane];
            }
        }
    }

    // ------------------------------------------------------------ clipping --

    /// `clipmacro`: move `v1`'s coordinate to the clip line `wl`, along the
    /// line to `(v2, w2)`, in the source's 16/32-bit arithmetic.
    fn clip_endpoint(v1: &mut i16, w1: &mut i16, v2: i16, w2: i16, wl: i16) {
        if *w1 != w2 {
            let num = (i32::from(v2) - i32::from(*v1)) * (i32::from(wl) - i32::from(*w1));
            let q = num / (i32::from(w2) - i32::from(*w1));
            *v1 = (*v1 as i32).wrapping_add(q) as i16;
        }
        *w1 = wl;
    }

    /// `clipliney`: clip the line in `clip_*` against y = 0 and 199, or reject
    /// it. `None` is the source's non-zero return.
    fn clipliney(&mut self) -> Option<()> {
        let (y1, y2) = (self.clip_y1, self.clip_y2);
        let mut bl = 0u8;
        if y1 < 0 {
            bl |= 4;
        } else if y1 > 199 {
            bl |= 8;
        }
        let mut bh = 0u8;
        if y2 < 0 {
            bh |= 4;
        } else if y2 > 199 {
            bh |= 8;
        }
        if bl & bh != 0 {
            return None;
        }
        if bl & 4 != 0 {
            Self::clip_endpoint(&mut self.clip_x1, &mut self.clip_y1, self.clip_x2, self.clip_y2, 0);
        }
        if bl & 8 != 0 {
            Self::clip_endpoint(
                &mut self.clip_x1,
                &mut self.clip_y1,
                self.clip_x2,
                self.clip_y2,
                199,
            );
        }
        if bh & 4 != 0 {
            Self::clip_endpoint(&mut self.clip_x2, &mut self.clip_y2, self.clip_x1, self.clip_y1, 0);
        }
        if bh & 8 != 0 {
            Self::clip_endpoint(
                &mut self.clip_x2,
                &mut self.clip_y2,
                self.clip_x1,
                self.clip_y1,
                199,
            );
        }
        Some(())
    }

    /// `cliplinex`: the same against x = 0 and 319.
    fn cliplinex(&mut self) -> Option<()> {
        let (x1, x2) = (self.clip_x1, self.clip_x2);
        let mut bl = 0u8;
        if x1 < 0 {
            bl |= 1;
        } else if x1 > 319 {
            bl |= 2;
        }
        let mut bh = 0u8;
        if x2 < 0 {
            bh |= 1;
        } else if x2 > 319 {
            bh |= 2;
        }
        if bl & bh != 0 {
            return None;
        }
        if bl & 1 != 0 {
            Self::clip_endpoint(&mut self.clip_y1, &mut self.clip_x1, self.clip_y2, self.clip_x2, 0);
        }
        if bl & 2 != 0 {
            Self::clip_endpoint(
                &mut self.clip_y1,
                &mut self.clip_x1,
                self.clip_y2,
                self.clip_x2,
                319,
            );
        }
        if bh & 1 != 0 {
            Self::clip_endpoint(&mut self.clip_y2, &mut self.clip_x2, self.clip_y1, self.clip_x1, 0);
        }
        if bh & 2 != 0 {
            Self::clip_endpoint(
                &mut self.clip_y2,
                &mut self.clip_x2,
                self.clip_y1,
                self.clip_x1,
                319,
            );
        }
        Some(())
    }

    /// `cap2r`: clip one line in y then x, writing `polyxy` and `polysides`.
    fn clip_line(&mut self) {
        if self.clipliney().is_none() || self.cliplinex().is_none() {
            self.polysides = 0;
            return;
        }
        self.polyxy[0] = (self.clip_x1, self.clip_y1);
        self.polyxy[1] = (self.clip_x2, self.clip_y2);
        self.polysides = if (self.clip_x1, self.clip_y1) == (self.clip_x2, self.clip_y2) {
            1
        } else {
            2
        };
    }

    /// `clipanypoly` (`POLYCLIP.ASM`), four points from `asmbox`.
    ///
    /// The closing edge `(last -> first)` is clipped **before** the loop and
    /// its result is the loop's first edge: `cap3` ends with `jmp cap35`,
    /// which skips the `cap32` clipping of the first iteration.
    fn clipanypoly(&mut self, pts: &[(i16, i16); 4]) {
        let mut out: Vec<(i16, i16)> = Vec::with_capacity(32);
        let mut edx = (i16::MIN, i16::MIN);
        let append = |out: &mut Vec<(i16, i16)>, edx: &mut (i16, i16),
                          a: (i16, i16), b: (i16, i16)| {
            if a != *edx {
                out.push(a);
                *edx = a;
            }
            if b != *edx {
                out.push(b);
                *edx = b;
            }
        };
        // The closing edge, clipped once, is the first edge.
        self.clip_x1 = pts[3].0;
        self.clip_y1 = pts[3].1;
        self.clip_x2 = pts[0].0;
        self.clip_y2 = pts[0].1;
        if self.clipliney().is_some() {
            let a = (self.clip_x1, self.clip_y1);
            let b = (self.clip_x2, self.clip_y2);
            append(&mut out, &mut edx, a, b);
        }
        for i in 1..4usize {
            self.clip_x1 = pts[i - 1].0;
            self.clip_y1 = pts[i - 1].1;
            self.clip_x2 = pts[i].0;
            self.clip_y2 = pts[i].1;
            if self.clipliney().is_some() {
                let a = (self.clip_x1, self.clip_y1);
                let b = (self.clip_x2, self.clip_y2);
                append(&mut out, &mut edx, a, b);
            }
        }
        for (i, point) in out.iter().enumerate() {
            if i < 32 {
                self.clipxy2[i] = *point;
            }
        }
        let mut cx = out.len();
        if cx > 0 && self.clipxy2[0] == edx {
            cx -= 1;
        }
        if cx == 0 {
            self.polysides = 0;
            return;
        }
        if cx <= 2 {
            // `cap2r`: reclip the remaining line (`clipxy2[4]` — here the
            // second slot — is the stale point when only one survived).
            self.clip_x1 = self.clipxy2[0].0;
            self.clip_y1 = self.clipxy2[0].1;
            self.clip_x2 = self.clipxy2[1].0;
            self.clip_y2 = self.clipxy2[1].1;
            self.clip_line();
            return;
        }
        // The x pass: the closing edge first, then the other edges.
        let mut out2: Vec<(i16, i16)> = Vec::with_capacity(32);
        let mut edx2 = (i16::MIN, i16::MIN);
        self.clip_x1 = self.clipxy2[cx - 1].0;
        self.clip_y1 = self.clipxy2[cx - 1].1;
        self.clip_x2 = self.clipxy2[0].0;
        self.clip_y2 = self.clipxy2[0].1;
        if self.cliplinex().is_some() {
            let a = (self.clip_x1, self.clip_y1);
            let b = (self.clip_x2, self.clip_y2);
            append(&mut out2, &mut edx2, a, b);
        }
        for i in 1..cx {
            self.clip_x1 = self.clipxy2[i - 1].0;
            self.clip_y1 = self.clipxy2[i - 1].1;
            self.clip_x2 = self.clipxy2[i].0;
            self.clip_y2 = self.clipxy2[i].1;
            if self.cliplinex().is_some() {
                let a = (self.clip_x1, self.clip_y1);
                let b = (self.clip_x2, self.clip_y2);
                append(&mut out2, &mut edx2, a, b);
            }
        }
        let mut n = out2.len();
        if n > 0 && out2[0] == edx2 {
            n -= 1;
        }
        self.polysides = n;
        for (i, point) in out2.iter().take(n).enumerate() {
            self.polyxy[i] = *point;
        }
    }

    /// `drawline` (`KOEA.ASM:170-370`): the source's y-sorted DDA with its
    /// unrolled mask states.
    fn drawline(&mut self, a: (i16, i16), b: (i16, i16)) {
        let (mut ax, mut bx) = (i32::from(a.0), i32::from(a.1));
        let (mut dx, mut cx) = (i32::from(b.0), i32::from(b.1));
        if bx == cx {
            return;
        }
        if bx > cx {
            std::mem::swap(&mut bx, &mut cx);
            std::mem::swap(&mut ax, &mut dx);
        }
        let dy = cx - bx;
        let mut si = (bx * 40 + (ax as u32 >> 3) as i32) & 0xFFFF;
        let right = ax < dx;
        let x_total = if right { dx - ax } else { ax - dx };
        let mut acc = -(dy >> 1);
        let mut state = ax & 7;
        let mut count = dy;
        loop {
            let at = (si & 0xFFFF) as usize;
            self.vbuf[at] ^= 0x80u8 >> state;
            si = (si + 40) & 0xFFFF;
            count -= 1;
            if count == 0 {
                return;
            }
            acc += x_total;
            if acc < 0 {
                continue;
            }
            loop {
                if right {
                    state = (state + 1) & 7;
                    if state == 0 {
                        si = (si + 1) & 0xFFFF;
                    }
                } else {
                    state = (state + 7) & 7;
                    if state == 7 {
                        si = (si - 1) & 0xFFFF;
                    }
                }
                acc -= dy;
                if acc < 0 {
                    break;
                }
            }
        }
    }

    /// `_asmbox` (`KOEA.ASM:373-419`): clip the four points and draw the
    /// polygon's edges; a one- or two-point result draws nothing (a dot is
    /// `y1 == y2`, and a two-point result draws the same line twice).
    fn asmbox(&mut self, pts: [(i16, i16); 4]) {
        self.polyisides = 4;
        self.clipanypoly(&pts);
        let n = self.polysides;
        if n == 0 {
            return;
        }
        if n == 1 {
            // The source draws `polyxy[0]` to itself; `drawline` sees equal
            // rows and returns.
            return;
        }
        for i in 0..n - 1 {
            self.drawline(self.polyxy[i], self.polyxy[i + 1]);
        }
        self.drawline(self.polyxy[n - 1], self.polyxy[0]);
    }

    // -------------------------------------------------------------- the fill --

    /// `blit16`/`blit16b`: the prefix-XOR fill into one plane.
    fn blit(&mut self, page: usize, mask: u8, row_step: usize) {
        let plane = (mask.trailing_zeros() as usize).min(3);
        for y in 0..usize::from(SCREEN_HEIGHT) {
            let mut carry = 0u8;
            for k in 0..40usize {
                let e = self.vbuf[y * 40 + k] ^ carry;
                let (lo, hi) = self.blit[e as usize];
                let at = (page + y * row_step + k) & (PLANE_BYTES - 1);
                self.planes[plane][at] = lo;
                carry = hi;
            }
        }
    }

    // ------------------------------------------------------------ the bars --

    /// One iteration of the first bar wipe's a-loop: the clear, then the
    /// flash's value is left pending for the next frame (the source's
    /// `dis_waitb` sits between the computation and the upload).
    fn wipe_iteration(&mut self) {
        let a = 256 - self.wipe_step * 32;
        let zly = self.wipe_zy;
        self.wipe_zya += 1;
        self.wipe_zy += self.wipe_zya;
        let band = self.wipe_band;
        let mut v = zly * 40;
        for _ in zly..self.wipe_zy {
            for k in 0..10usize {
                let at = (v as usize + band * 10 + k) & (PLANE_BYTES - 1);
                for plane in &mut self.planes {
                    plane[at] = 0;
                }
            }
            v += 40;
        }
        self.wipe_pending = Some(if a >= 0 { a } else { 0 });
    }
}

/// `rotate1` (`KOEA.ASM:605-627`): rotate the first 31,936 bytes right one bit
/// with a zero shifted in; the last 64 bytes of `dst` are untouched.
fn rotate1(src: &[u8], dst: &mut [u8]) {
    let mut carry = 0u8;
    for i in 0..32_000 - 64 {
        let value = src[i];
        dst[i] = (value >> 1) | (carry << 7);
        carry = value & 1;
    }
}

/// `KOE.C:623-633`: the sixteen troll fade tables, `palette + (45 - 3y)`.
fn troll_fade(palette: &[u8; 768]) -> Vec<u8> {
    let mut fade = vec![0u8; 16 * 768];
    for y in 0..16usize {
        let step = 45 - 3 * y as i32;
        for a in 0..768usize {
            fade[y * 768 + a] = (i32::from(palette[a]) + step).min(63) as u8;
        }
    }
    fade
}

/// `READP.C`: the Origin RLE row reader, with the source's 16-bit fields and
/// its in-place header hack. A row is the RLE opcodes: `>= 0x80` repeats the
/// next byte `opcode & 0x7f` times, anything else is a literal.
///
/// The runtime uses `sr_assets`' Origin reader; this is the literal
/// transcription, and the test below checks the two agree row for row.
#[cfg(test)]
fn readp_rows(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut buf = bytes.to_vec();
    buf.resize(80_000, 0);
    let height = i16::from_le_bytes([buf[4], buf[5]]);
    let colrs = i16::from_le_bytes([buf[6], buf[7]]);
    let add = i16::from_le_bytes([buf[8], buf[9]]);
    let _ = colrs;
    let mut rows = Vec::new();
    for row in 0..height.max(0) as usize {
        let mut src = (add.max(0) as usize) * 16;
        for _ in 0..row {
            let delta = i16::from_le_bytes([buf[src], buf[src + 1]]) as i32;
            src = (src as i32 + delta + 2) as usize;
            let flag = u16::from_le_bytes([buf[src], buf[src + 1]]);
            if flag >= 0x4000 {
                let new = flag - 0x4000;
                buf[src..src + 2].copy_from_slice(&new.to_le_bytes());
                let next = u16::from_le_bytes([buf[src + 2], buf[src + 3]]);
                buf[src + 2..src + 4].copy_from_slice(&next.wrapping_add(0x400).to_le_bytes());
            }
        }
        let count = i16::from_le_bytes([buf[src], buf[src + 1]]) as i32;
        src += 2;
        let end = (src as i32 + count.max(0)) as usize;
        let mut out = Vec::with_capacity(320);
        while src < end && src < buf.len() {
            let op = buf[src];
            src += 1;
            if op & 0x80 != 0 {
                let repeat = usize::from(op & 0x7F);
                let value = if src < buf.len() { buf[src] } else { 0 };
                src += 1;
                out.extend(std::iter::repeat_n(value, repeat));
            } else {
                out.push(op);
            }
        }
        rows.push(out);
    }
    rows
}

impl Part for Techno {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        if ctx.music().is_none() {
            return Err(PartError {
                part: NAME,
                message: "this part waits on the music player's position \
                          (TECHNO/KOE.C:213, :275, KOEA.ASM:900) and the run has no music \
                          to wait on"
                    .into(),
            });
        }
        let read = |path: &str| -> Result<Vec<u8>, PartError> {
            let path = format!("{}{path}", self.root);
            std::fs::read(&path).map_err(|error| PartError {
                part: NAME,
                message: format!("{path}: {error}"),
            })
        };
        self.circle2 = read(CIRCLE2_PATH)?;
        self.circle = read(CIRCLE_PATH)?;
        if self.circle2.len() != 8_000 {
            return Err(PartError {
                part: NAME,
                message: format!("{CIRCLE2_PATH} is {} bytes, not 8000", self.circle2.len()),
            });
        }
        if self.circle.len() != 24_000 {
            return Err(PartError {
                part: NAME,
                message: format!("{CIRCLE_PATH} is {} bytes, not 24000", self.circle.len()),
            });
        }
        let troll_path = format!("{}{TROLL_PATH}", self.root);
        let troll: Image = load_path(Path::new(&troll_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{troll_path}: {error}"),
        })?;
        if (troll.width(), troll.height()) != (SCREEN_WIDTH, TROLL_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{TROLL_PATH} is {}x{}, not 320x400",
                    troll.width(),
                    troll.height()
                ),
            });
        }
        self.troll_pixels = troll.pixels().to_vec();
        for entry in 0..=255u8 {
            let rgb = troll.palette.entry(entry);
            let at = usize::from(entry) * 3;
            self.troll_palette[at] = rgb[0];
            self.troll_palette[at + 1] = rgb[1];
            self.troll_palette[at + 2] = rgb[2];
        }
        self.palfade = troll_fade(&self.troll_palette);
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
            // The preamble's six emulator frames over three ticks.
            self.extra_polls = 2;
        }
        self.phase = Phase::Gate;
        self.tick = 0;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.exit_requested {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        self.tick += 1;
        if self.tick > 0 && self.extra_polls > 0 {
            if let Some(music) = ctx.music() {
                music.poll();
            }
            self.extra_polls -= 1;
        }
        match self.phase {
            Phase::Gate => {
                let musplus = ctx.music().map_or(-32, |music| music.musplus());
                if musplus < GATE_ONE {
                    return PartStatus::Continuing;
                }
                if let Some(music) = ctx.music() {
                    music.setmframe(0);
                }
                self.phase = Phase::ResetClear;
            }
            Phase::ResetClear => {
                self.dac = [0; 768];
                self.phase = Phase::ResetMode;
            }
            Phase::ResetMode => {
                // `resetmode13`: mode 13h clears the video memory and reloads
                // the BIOS palette; the second DAC clear follows.
                for plane in &mut self.planes {
                    plane.fill(0);
                }
                self.dac = [0; 768];
                self.pan = 0;
                self.display_start = 0;
                self.stride = PLANE_ROW_WIDE;
                self.p1_scrnpos = Some(0);
                self.p1_scrnposl = 0;
                self.init_interference();
                self.phase = Phase::FirstPass;
            }
            Phase::FirstPass => {
                let row = ctx.music().map_or(0, |music| music.musrow());
                self.first_pass_iteration(row);
                if self.p1_framecount >= 256 {
                    self.init_interference();
                    self.p2_first_frame = true;
                    self.phase = Phase::SecondPass;
                }
            }
            Phase::SecondPass => {
                let row = ctx.music().map_or(0, |music| music.musrow());
                self.second_pass_iteration(row);
                let mframe = ctx.music().map_or(0, |music| music.getmframe());
                if mframe >= INTERFERENCE_END {
                    self.flash_capture();
                    self.phase = Phase::Flash32;
                }
            }
            Phase::Flash32 => {
                self.flash_apply(32);
                self.phase = Phase::Flash64;
            }
            Phase::Flash64 => {
                self.flash_apply(64);
                self.phase = Phase::Flash192;
            }
            Phase::Flash192 => {
                self.flash_apply(192);
                self.phase = Phase::Flash256;
            }
            Phase::Flash256 => {
                self.flash_apply(256);
                self.mode16();
                self.phase = Phase::MusplusMinus3;
            }
            Phase::MusplusMinus3 => {
                let musplus = ctx.music().map_or(-32, |music| music.musplus());
                if musplus < GATE_TWO {
                    return PartStatus::Continuing;
                }
                self.phase = Phase::RowWait;
                return self.row_wait(ctx);
            }
            Phase::RowWait => return self.row_wait(ctx),
            Phase::Wipe => {
                if let Some(flash) = self.wipe_pending.take() {
                    self.flash_apply(flash);
                }
                self.wipe_iteration();
                self.wipe_step += 1;
                if self.wipe_step >= 21 {
                    self.phase = Phase::WipeRowWait;
                }
            }
            Phase::WipeRowWait => {
                if let Some(flash) = self.wipe_pending.take() {
                    self.flash_apply(flash);
                }
                let row = ctx.music().map_or(0, |music| music.musrow());
                if row & 7 != 7 {
                    return PartStatus::Continuing;
                }
                self.flash_capture();
                self.wipe_pending = Some(32);
                self.wipe_flash = 0;
                self.phase = Phase::WipeFlash;
            }
            Phase::WipeFlash => {
                if let Some(flash) = self.wipe_pending.take() {
                    self.flash_apply(flash);
                }
                if self.wipe_flash < 3 {
                    let next = [64, 192, 256][usize::from(self.wipe_flash)];
                    self.wipe_pending = Some(next);
                    self.wipe_flash += 1;
                } else {
                    // `flash(256)` has uploaded; the b-loop ends here.
                    self.wipe_band += 1;
                    if self.wipe_band >= 4 {
                        // `doit1`'s first `waitborder` reads the row now and
                        // waits; the loop's next frame uploads it.
                        let row = ctx.music().map_or(0, |music| music.musrow());
                        self.bar_read(row);
                        self.phase = Phase::Doit1;
                        self.doit_first = true;
                        self.doit_count = DOIT1_COUNT;
                        self.rot = 45;
                        self.vm = 50;
                        self.vma = 0;
                        self.plv = 0;
                        self.pl = 1;
                        self.page = 0;
                    } else {
                        self.wipe_step = 0;
                        self.wipe_zy = 0;
                        self.wipe_zya = 0;
                        self.phase = Phase::Wipe;
                        // The next band's first a-iteration clears in this
                        // same frame (the upload above was `flash(256)`).
                        self.wipe_iteration();
                        self.wipe_step += 1;
                        if self.wipe_step >= 21 {
                            self.phase = Phase::WipeRowWait;
                        }
                    }
                }
            }
            Phase::Doit1 => return self.doit_tick(ctx, 1),
            Phase::Doit2 => return self.doit_tick(ctx, 2),
            Phase::Doit3Prologue => return self.doit3_prologue(ctx),
            Phase::Doit3 => return self.doit3_tick(ctx),
            Phase::TrollWait1 => {
                self.phase = Phase::TrollWait2;
            }
            Phase::TrollWait2 => {
                self.phase = Phase::TrollBuild;
            }
            Phase::TrollBuild => {
                self.build_troll(ctx);
                self.phase = Phase::TrollPalette;
            }
            Phase::TrollPalette => {
                let palette = self.troll_palette;
                self.set_pal_area(&palette, 0, 256);
                self.phase = Phase::TrollOrderWait;
                return self.troll_order_wait(ctx);
            }
            Phase::TrollOrderWait => return self.troll_order_wait(ctx),
            Phase::TrollPan => return self.troll_pan(ctx),
            Phase::TrollRipple => return self.troll_ripple(ctx),
            Phase::TrollFinalWait => return self.troll_final_wait(ctx),
            Phase::Done => return PartStatus::Finished,
        }
        self.present(ctx);
        PartStatus::Continuing
    }
}

impl Techno {
    /// The bar wipe's per-band state reset and the first a-iteration's clear.
    fn wipe_band_set(&mut self) {
        self.wipe_zy = 0;
        self.wipe_zya = 0;
        self.wipe_step = 0;
    }

    /// `KOE.C:277-281`: wait for `musrow & 7 == 7`, then start the first bar.
    fn row_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if let Some(flash) = self.wipe_pending.take() {
            self.flash_apply(flash);
        }
        let row = ctx.music().map_or(0, |music| music.musrow());
        if row & 7 != 7 {
            return PartStatus::Continuing;
        }
        self.wipe_band = 0;
        self.wipe_band_set();
        self.wipe_flash = 0;
        self.phase = Phase::Wipe;
        // The first a-iteration runs in the same frame the wait passes.
        self.wipe_iteration();
        self.wipe_step += 1;
        if self.wipe_step >= 21 {
            self.phase = Phase::WipeRowWait;
        }
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:236-273`: the 16-colour mode set, the clear, the white fill and
    /// the dead `ip` build.
    fn mode16(&mut self) {
        self.stride = PLANE_ROW;
        self.display_start = 0;
        self.pan = 0;
        // `memset(vram,0,64000)` (`KOE.C:259`): the last 1,536 bytes of each
        // plane keep what the interference left, exactly as the source does.
        for plane in &mut self.planes {
            plane[..64_000].fill(0);
        }
        for plane in &mut self.planes {
            plane[..PAGE_BYTES].fill(0xFF);
        }
        // `ip = flash(-2)` returns `pal1`; the loop writes it and nothing
        // uploads it. Kept as the source's dead write.
        for a in 0..16usize {
            self.flash_pal[a * 3] = a as i16 * 6 / 2;
            self.flash_pal[a * 3 + 1] = a as i16 * 7 / 2;
            self.flash_pal[a * 3 + 2] = a as i16 * 8 / 2;
        }
    }

    /// One tick of doit1 (`which = 1`) or doit2.
    fn doit_tick(&mut self, ctx: &mut PartCtx, which: u8) -> PartStatus {
        if self.doit_first {
            self.bar_upload_and_read(ctx);
            self.doit_first = false;
            return PartStatus::Continuing;
        }
        self.bar_upload_and_read(ctx);
        self.doit_count -= 1;
        let mut vbuf = std::mem::take(&mut self.vbuf);
        vbuf.fill(0);
        self.vbuf = vbuf;
        self.doit_boxes(which);
        let page = self.page;
        let mask = self.pl as u8;
        self.blit(page, mask, PLANE_ROW);
        self.display_start = (self.plv * PAGE_HIGH) << 8;
        self.plv = (self.plv + 1) & 7;
        self.page = self.plv as usize * PAGE_STEP;
        if self.plv == 0 {
            self.pl <<= 1;
            if self.pl > 15 {
                self.pl = 1;
            }
        }
        if self.doit_count <= 0 {
            if which == 1 {
                self.phase = Phase::Doit2;
                self.doit_count = DOIT2_COUNT;
                self.doit_first = true;
                // `doit2`'s own locals.
                self.rot = 50;
                self.rota = 10;
                self.vm = 100 * 64;
                self.vma = 0;
                self.plv = 0;
                self.pl = 1;
                self.page = 0;
            } else {
                self.phase = Phase::Doit3Prologue;
                self.doit_count = DOIT3_COUNT;
                self.doit_first = true;
            }
        }
        self.present(ctx);
        PartStatus::Continuing
    }

    /// doit1's and doit2's boxes (`KOE.C:355-379`, `:414-438`).
    fn doit_boxes(&mut self, which: u8) {
        if which == 1 {
            let hx = (SIN_TABLE[self.rot as usize & 1023] as i32) * 16 * 6 / 5;
            let hy = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 16;
            let vx = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 6 / 5;
            let vy = SIN_TABLE[(self.rot + 512) as usize & 1023] as i32;
            let vx = vx * self.vm / 100;
            let vy = vy * self.vm / 100;
            self.boxes(hx, hy, vx, vy, 160, 100);
            self.rot += 2;
            self.vm += self.vma;
            if self.vm < 25 {
                self.vm -= self.vma;
                self.vma = -self.vma;
            }
            self.vma -= 1;
        } else {
            let hx = (SIN_TABLE[self.rot as usize & 1023] as i32) * 16 * 6 / 5;
            let hy = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 16;
            let vx = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 6 / 5;
            let vy = SIN_TABLE[(self.rot + 512) as usize & 1023] as i32;
            let vx = vx * (self.vm / 64) / 100;
            let vy = vy * (self.vm / 64) / 100;
            self.boxes(hx, hy, vx, vy, 160, 100);
            self.rot += self.rota / 10;
            self.vm += self.vma;
            if self.vm < 0 {
                self.vm -= self.vma;
                self.vma = -self.vma;
            }
            self.vma -= 1;
            self.rota += 1;
        }
    }

    /// The eleven parallelograms a `doit` frame draws.
    fn boxes(&mut self, hx: i32, hy: i32, vx: i32, vy: i32, cx0: i32, cy0: i32) {
        for c in (-10..11).step_by(2) {
            let cx = vx * c * 2;
            let cy = vy * c * 2;
            let x1 = (-hx - vx + cx) / 16 + cx0;
            let y1 = (-hy - vy + cy) / 16 + cy0;
            let x2 = (-hx + vx + cx) / 16 + cx0;
            let y2 = (-hy + vy + cy) / 16 + cy0;
            let x3 = (hx + vx + cx) / 16 + cx0;
            let y3 = (hy + vy + cy) / 16 + cy0;
            let x4 = (hx - vx + cx) / 16 + cx0;
            let y4 = (hy - vy + cy) / 16 + cy0;
            self.asmbox([
                (x1 as i16, y1 as i16),
                (x2 as i16, y2 as i16),
                (x3 as i16, y3 as i16),
                (x4 as i16, y4 as i16),
            ]);
        }
    }

    /// `waitborder`, split: the upload of the block the previous read armed.
    fn bar_upload_and_read(&mut self, ctx: &mut PartCtx) {
        self.upload_bar();
        let row = ctx.music().map_or(0, |music| music.musrow());
        self.bar_read(row);
    }

    /// doit3's prologue tick: the bar upload, the mode set, then the loop's
    /// first pre-wait part.
    fn doit3_prologue(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.upload_bar();
        let row = ctx.music().map_or(0, |music| music.musrow());
        self.bar_read(row);
        // `doit3`'s prologue.
        self.dac[..48].fill(0);
        for plane in &mut self.planes {
            plane.fill(0);
        }
        self.display_start = 0;
        self.stride = PLANE_ROW_WIDE;
        self.pan = 0;
        self.rot = 45;
        self.rota = 10;
        self.rot2 = 0;
        self.vm = 100 * 64;
        self.vma = 0;
        self.xpos = 320;
        self.xposa = 0;
        self.repeat = 1;
        self.plv = 0;
        self.pl = 1;
        self.page = 40;
        self.doit_first = false;
        self.repeat = 1;
        // The loop's first pre-wait part.
        if self.doit3_pre(ctx) {
            return self.doit3_finish(ctx);
        }
        self.phase = Phase::Doit3;
        self.present(ctx);
        PartStatus::Continuing
    }

    /// doit3's per-frame tick: the previous iteration's post-wait drawing,
    /// then the next iteration's pre-wait part.
    fn doit3_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.upload_bar();
        self.doit_count -= 1;
        self.repeat = 1;
        self.doit3_post();
        if self.doit3_pre(ctx) {
            return self.doit3_finish(ctx);
        }
        self.present(ctx);
        PartStatus::Continuing
    }

    /// The post-wait half of a doit3 iteration.
    fn doit3_post(&mut self) {
        // The 16-colour screen's ATC pan is `xpos & 7` pixels (`KOE.C:528`);
        // the `(xpos & 3)*2` form is the troll's 256-colour one. With the
        // start register at `xpos/8` plane bytes the total is
        // `8*(xpos/8) + (xpos & 7) = xpos`.
        self.pan = (self.xpos & 7) as u8;
        let mut vbuf = std::mem::take(&mut self.vbuf);
        vbuf.fill(0);
        self.vbuf = vbuf;
        let (wx, wy) = (self.doit_wx, self.doit_wy);
        let hx = (SIN_TABLE[self.rot as usize & 1023] as i32) * 16 * 6 / 5;
        let hy = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 16;
        let vx = (SIN_TABLE[(self.rot + 256) as usize & 1023] as i32) * 6 / 5;
        let vy = SIN_TABLE[(self.rot + 512) as usize & 1023] as i32;
        let vx = vx * (self.vm / 64) / 100;
        let vy = vy * (self.vm / 64) / 100;
        self.boxes(hx, hy, vx, vy, wx, wy);
        self.rot += self.rota / 10;
        self.vm += self.vma;
        if self.vm < 0 {
            self.vm -= self.vma;
            self.vma = -self.vma;
        }
        self.vma -= 1;
        self.rota += 1;
        let page = self.page;
        let mask = self.pl as u8;
        self.blit(page, mask, PLANE_ROW_WIDE);
        self.display_start = (self.plv * PAGE_HIGH) << 8 | (self.display_start & 0xFF);
        self.plv = (self.plv + 2) & 7;
        self.page = self.plv as usize * PAGE_STEP + 40;
        if self.plv == 0 {
            self.pl <<= 1;
            if self.pl > 15 {
                self.pl = 1;
            }
        }
    }

    /// The pre-wait half of a doit3 iteration. `true` when the loop breaks.
    fn doit3_pre(&mut self, ctx: &mut PartCtx) -> bool {
        let mframe = ctx.music().map_or(0, |music| music.getmframe());
        if mframe > DOIT3_MFRAME {
            if self.xpos != 0 {
                self.xpos = 0;
            } else {
                return true;
            }
        }
        if self.doit_count < DOIT3_COUNTDOWN {
            while self.repeat > 0 {
                self.repeat -= 1;
                self.xpos -= self.xposa / 4;
                if self.xpos < 0 {
                    self.xpos = 0;
                } else {
                    self.xposa += 1;
                }
            }
            if self.xpos == 0 {
                return true;
            }
        }
        if self.rot2 < 32 {
            self.doit_wx = (SIN_TABLE[self.rot2 as usize & 1023] as i32) * self.rot2 / 8 + 160;
            self.doit_wy =
                (SIN_TABLE[(self.rot2 + 256) as usize & 1023] as i32) * self.rot2 / 8 + 100;
        } else {
            // C's `/4` truncates toward zero; an arithmetic shift would
            // floor the negative half of the sine and move every edge a
            // pixel.
            self.doit_wx = (SIN_TABLE[self.rot2 as usize & 1023] as i32) / 4 + 160;
            self.doit_wy = (SIN_TABLE[(self.rot2 + 256) as usize & 1023] as i32) / 4 + 100;
        }
        self.rot2 += 17;
        self.display_start = (self.display_start & 0xFF00) | (self.xpos / 8);
        let row = ctx.music().map_or(0, |music| music.musrow());
        self.bar_read(row);
        false
    }

    /// The mode 13h / inittwk transition and the first troll tick's wait.
    fn doit3_finish(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `mov ax,13h; int 10h`: the video memory is cleared and the DAC
        // reloads the BIOS palette; `inittwk` clears both again.
        for plane in &mut self.planes {
            plane.fill(0);
        }
        self.dac = [0; 768];
        self.stride = TROLL_ROW;
        self.display_start = 0;
        self.pan = 0;
        self.page = 0;
        // The surface is the interleaved four-plane image.
        ctx.surface.pixels_mut().fill(0);
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = TROLL_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(640);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
        self.phase = Phase::TrollWait1;
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:614-636`: the two reads, the RLE decode, the plane blits and the
    /// fade tables.
    fn build_troll(&mut self, _ctx: &mut PartCtx) {
        for y in 0..400usize {
            let row: [u8; 320] = self.troll_pixels[y * 320..(y + 1) * 320].try_into().unwrap();
            self.lineblit(&row, y);
        }
        // The interleaved image is the surface's 320..639 of every row.
        let _ = &self.palfade;
    }

    /// `KOE.C:638-648`: the order/row wait.
    fn troll_order_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let (order, row) = ctx
            .music()
            .map_or((0, 0), |music| {
                let position = music.position();
                (position.order, position.row)
            });
        if order > 35 || (order == 35 && row > 48) {
            self.xpos = 0;
            self.xposa = 0;
            self.doit_count = 300;
            self.phase = Phase::TrollPan;
            return self.troll_pan(ctx);
        }
        // The screen holds; the picture and palette do not change.
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:649-673`: the 300-count pan in from the left.
    fn troll_pan(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.xpos == 320 {
            self.ripple = 0;
            self.ripplep = 8;
            self.ripple_c = 0;
            self.doit_count = 50;
            self.phase = Phase::TrollRipple;
            return self.troll_ripple(ctx);
        }
        self.xpos += self.xposa / 4;
        if self.xpos > 320 {
            self.xpos = 320;
        } else {
            self.xposa += 1;
        }
        self.troll_view(ctx);
        self.doit_count -= 1;
        if self.doit_count <= 0 {
            self.ripple = 0;
            self.ripplep = 8;
            self.ripple_c = 0;
            self.doit_count = 50;
            self.phase = Phase::TrollRipple;
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:674-703`: the 50-frame ripple and its sixteen fade steps.
    fn troll_ripple(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.ripplep > 1023 {
            self.ripplep = 1024;
        } else {
            self.ripplep = self.ripplep * 5 / 4;
        }
        self.xpos = 320 + (SIN_TABLE[self.ripple as usize & 1023] as i32) / self.ripplep;
        self.ripple += self.ripplep + 100;
        if self.ripple_c < 16 {
            let fade = self.palfade.clone();
            let at = self.ripple_c as usize * 768;
            self.set_pal_area(&fade[at..at + 768], 0, 256);
            self.ripple_c += 1;
        }
        self.troll_view(ctx);
        self.doit_count -= 1;
        if self.doit_count <= 0 {
            let palette = self.troll_palette;
            self.set_pal_area(&palette, 0, 256);
            self.xpos = 320;
            self.doit_count = 420;
            self.phase = Phase::TrollFinalWait;
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:705-726`: the final music wait at the picture's held position.
    fn troll_final_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let musplus = ctx.music().map_or(-32, |music| music.musplus());
        if musplus > -6 && musplus < 16 {
            self.phase = Phase::Done;
            self.troll_compose(ctx);
            self.present(ctx);
            return PartStatus::Finished;
        }
        self.troll_view(ctx);
        self.doit_count -= 1;
        if self.doit_count <= 0 {
            self.phase = Phase::Done;
            self.troll_compose(ctx);
            self.present(ctx);
            return PartStatus::Finished;
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// The troll's CRTC state: start low = xpos/4 (plane bytes), ATC pan
    /// halved by the 256-colour mode.
    fn troll_view(&mut self, ctx: &mut PartCtx) {
        self.display_start = self.xpos / 4;
        self.pan = ((self.xpos & 3) * 2) as u8;
        ctx.view.start_addr = (self.display_start * 4) as u32;
        ctx.view.x_offset = 0;
        ctx.view.pan = ((self.pan as u32 & 7) / 2) as u8;
        ctx.view.y_line_offset = Some(640);
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = TROLL_HEIGHT;
    }

    /// The troll's picture: the interleaved four-plane image in the surface.
    fn troll_compose(&self, ctx: &mut PartCtx) {
        let surface = ctx.surface.pixels_mut();
        for y in 0..400usize {
            let row = &self.troll_pixels[y * 320..(y + 1) * 320];
            let at = y * 640 + 320;
            surface[at..at + 320].copy_from_slice(row);
        }
    }

    /// Put this tick's DAC and screen on the engine.
    fn present(&self, ctx: &mut PartCtx) {
        let mut palette = Palette::new();
        for entry in 0..256usize {
            palette.set_entry(
                entry as u8,
                [
                    self.dac[entry * 3],
                    self.dac[entry * 3 + 1],
                    self.dac[entry * 3 + 2],
                ],
            );
        }
        ctx.set_palette(palette);
        if self.phase != Phase::Done && self.stride == TROLL_ROW {
            return;
        }
        if self.stride != TROLL_ROW {
            ctx.view.width = SCREEN_WIDTH;
            ctx.view.height = SCREEN_HEIGHT;
            ctx.view.double_scan = false;
            ctx.view.set_origin(0);
            ctx.view.y_line_offset = None;
            ctx.view.pan = 0;
            ctx.view.border = Some(0);
            ctx.view.split = None;
            self.compose_ega(ctx.surface.pixels_mut());
        }
    }
}

// A scratch field: the first pass's `scrnpos` is an `Option` only so the MOVE
// can be written against the source's own globals; it is always `Some` after
// `ResetMode`.
impl Techno {
    #[cfg(test)]
    fn at(root: &'static str) -> Techno {
        let mut part = Techno::new();
        part.root = root;
        part
    }

    /// A test's blank video memory: four empty planes and the 320×200 window
    /// at its first bytes.
    #[cfg(test)]
    fn reset_video_for_tests(&mut self) {
        for plane in &mut self.planes {
            plane.fill(0);
        }
        self.stride = PLANE_ROW_WIDE;
        self.display_start = 8020;
        self.pan = 0;
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::view::View;

    /// A real-player run to the end, recording where each phase begins. This
    /// is the timeline the reference keyframes pin; it is kept as a test so
    /// the section ticks are reproducible.
    struct Rig {
        surface: sr_engine::Surface,
        view: View,
        palette: Palette,
        timeline: sr_engine::palette::PaletteTimeline,
        copper: sr_engine::copper::CopperRegistry,
        music: crate::audio::PlayerMusic,
    }

    impl Rig {
        fn new_real() -> Rig {
            use crate::audio::{lock, test_player, PlayerMusic};
            use crate::shell::ModuleId;
            let player = test_player(ModuleId::Music1).unwrap();
            lock(&player).play_song(18);
            Rig {
                surface: sr_engine::Surface::new(640, 400),
                view: View::new(320, 200),
                palette: Palette::new(),
                timeline: sr_engine::palette::PaletteTimeline::new(Palette::new()),
                copper: sr_engine::copper::CopperRegistry::new(),
                music: PlayerMusic::new(player, 0),
            }
        }

        fn tick(&mut self, part: &mut Techno) -> PartStatus {
            use sr_engine::music::Music;
            self.music.poll();
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(&mut self.music),
                0,
                0,
                1,
                true,
                false,
                false,
            );
            part.frame(&mut ctx)
        }
    }

    #[test]
    fn the_sections_land_on_the_reference_ticks() {
        // Every phase change over the part's whole timeline, with the real
        // music player. The ticks are the ones the reference keyframes pin
        // (`docs/fidelity/techno.md`): 6300 is drawn at 1501 and 8100 at
        // 3301, inside Doit1 (from 1437) and Doit3 (from 2700); the troll's
        // mode change begins at 3401 and the picture holds from 3548.
        let mut rig = Rig::new_real();
        let mut part = Techno::at("../../");
        {
            let mut ctx = PartCtx::new(
                &mut rig.surface,
                &mut rig.view,
                &mut rig.palette,
                &mut rig.timeline,
                &mut rig.copper,
                Some(&mut rig.music),
                0,
                0,
                1,
                true,
                false,
                true,
            );
            part.init(&mut ctx).unwrap();
        }
        // A frame's index is the dump's index: `frame` increments `tick`
        // from the value set here, so the first call (tick 0) is the run's
        // frame 0 and the phase it leaves is that frame's.
        let mut transitions: Vec<(u32, Phase)> = Vec::new();
        let mut last = part.phase;
        for tick in 0..4200u32 {
            part.tick = tick;
            let status = rig.tick(&mut part);
            if part.phase != last {
                transitions.push((tick, part.phase));
                last = part.phase;
            }
            if status == PartStatus::Finished {
                break;
            }
        }
        use Phase::*;
        assert_eq!(
            transitions,
            vec![
                (0, ResetClear),
                (1, ResetMode),
                (2, FirstPass),
                (258, SecondPass),
                (1246, Flash32),
                (1247, Flash64),
                (1248, Flash192),
                (1249, Flash256),
                (1250, MusplusMinus3),
                (1295, RowWait),
                (1303, Wipe),
                (1323, WipeRowWait),
                (1335, WipeFlash),
                (1339, Wipe),
                (1359, WipeRowWait),
                (1367, WipeFlash),
                (1371, Wipe),
                (1391, WipeRowWait),
                (1400, WipeFlash),
                (1404, Wipe),
                (1424, WipeRowWait),
                (1432, WipeFlash),
                (1436, Doit1),
                (1857, Doit2),
                (2698, Doit3Prologue),
                (2699, Doit3),
                (3396, TrollWait1),
                (3397, TrollWait2),
                (3398, TrollBuild),
                (3399, TrollPalette),
                (3400, TrollOrderWait),
                (3445, TrollPan),
                (3498, TrollRipple),
                (3547, TrollFinalWait),
                (3967, Done),
            ]
        );
        // The two keyframe ticks are inside the sections their mappings name.
        assert_eq!(
            transitions.iter().rfind(|(t, _)| *t <= 1501).unwrap().1,
            Doit1
        );
        assert_eq!(
            transitions.iter().rfind(|(t, _)| *t <= 3301).unwrap().1,
            Doit3
        );
    }

    #[test]
    fn the_sin_table_is_the_incs_1024_words() {
        // `interference_draw`'s cursor is a byte offset into this word table
        // (`and bp,2047` = 1,024 words); the MOVE routines use a word index.
        let inc = std::fs::read_to_string("../../upstream/SecondReality/TECHNO/SIN1024.INC").unwrap();
        let body = inc.split("_sin1024 LABEL WORD").nth(1).unwrap();
        let words: Vec<i16> = body
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter_map(|token| token.parse::<i16>().ok())
            .take(1024)
            .collect();
        assert_eq!(words.len(), 1024);
        assert_eq!(SIN_TABLE.as_slice(), words.as_slice());
        assert_eq!(SIN_TABLE.iter().copied().max(), Some(256), "scaled to 256");
        assert_eq!(SIN_TABLE[0], 0);
    }

    #[test]
    fn the_two_geometries_are_the_parts_two_screens() {
        let mut rig = Rig::new_real();
        let mut part = Techno::at("../../");
        {
            let mut ctx = PartCtx::new(
                &mut rig.surface,
                &mut rig.view,
                &mut rig.palette,
                &mut rig.timeline,
                &mut rig.copper,
                Some(&mut rig.music),
                0,
                0,
                1,
                true,
                false,
                true,
            );
            part.init(&mut ctx).unwrap();
        }
        // The interference screen is 320×200 from the mode set to the troll.
        // The call with `tick` is engine frame `tick`; the view changes on
        // frame 3396 (the dump's geometry follows at 3397).
        for tick in 0..3396u32 {
            part.tick = tick;
            rig.tick(&mut part);
            assert_eq!((rig.view.width, rig.view.height), (320, 200), "tick {tick}");
        }
        // The mode 13h / inittwk transition sets the 320×400 view.
        for tick in 3396..3402u32 {
            part.tick = tick;
            rig.tick(&mut part);
        }
        assert_eq!((rig.view.width, rig.view.height), (320, 400));
        assert_eq!(rig.view.y_line_offset, Some(640), "a 160-byte plane row ×4");
        assert_eq!(rig.view.start_addr, 0, "the picture slides in from x = 0");
    }

    #[test]
    fn the_waitborder_slides_the_bar_blocks_and_resets_on_row_seven() {
        let mut t = Techno::new();
        // The first read sees a row that is not 7: block 0 holds.
        t.bar_read(6);
        assert_eq!(t.curpal, 0);
        t.upload_bar();
        assert_eq!(t.curpal, 0, "block 0 holds");
        // A row change to 7 arms block 15; every upload then slides down.
        t.bar_read(7);
        assert_eq!(t.curpal, 15);
        for i in 0..18usize {
            // `upload_bar` stops decrementing at 0, so the last three reads
            // are block 0 again (the slide is 16 blocks long, not 18).
            let block = 15usize.saturating_sub(i);
            let want = [
                t.bar_palette[block * 48],
                t.bar_palette[block * 48 + 1],
                t.bar_palette[block * 48 + 2],
            ];
            t.upload_bar();
            assert_eq!(&t.dac[..3], &want, "upload {i} is block {block}");
        }
        assert_eq!(t.curpal, 0, "it stops at block 0");
    }

    #[test]
    fn the_bar_palette_is_the_popcount_base_scaled_by_the_sources_factors() {
        let pal = bar_palette();
        let base = [
            [0i32, 0, 0],
            [38 * 64 / 111, 33 * 64 / 111, 44 * 64 / 111],
            [52 * 64 / 111, 45 * 64 / 111, 58 * 64 / 111],
            [67 * 64 / 111, 61 * 64 / 111, 73 * 64 / 111],
            [83 * 64 / 111, 77 * 64 / 111, 89 * 64 / 111],
        ];
        for c in 0..16i32 {
            for a in 0..16usize {
                let x = a.count_ones() as usize;
                let [r, g, b] = base[x];
                let want = [
                    (r * (10 + c * 9 / 9) / 10).min(63) as u8,
                    (g * (10 + c * 7 / 9) / 10).min(63) as u8,
                    (b * (10 + c * 5 / 9) / 10).min(63) as u8,
                ];
                let at = (c as usize * 16 + a) * 3;
                assert_eq!(&pal[at..at + 3], &want, "c={c} a={a}");
            }
        }
        // Pinned samples by hand.
        assert_eq!(&pal[3..6], &[21, 19, 25], "c=0 a=1 is the base triple");
        assert_eq!(&pal[(15 * 16 + 1) * 3..(15 * 16 + 1) * 3 + 3], &[52, 39, 45]);
        assert_eq!(&pal[(2 * 16 + 15) * 3..(2 * 16 + 15) * 3 + 3], &[56, 48, 56]);
    }

    #[test]
    fn the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512() {
        let mut t = Techno::new();
        t.dac = [7; 768];
        let table = vec![9u8; 768];
        t.set_pal_area(&table, 0, 256);
        assert_eq!(&t.dac[..512], &[9u8; 512], "2*256 + 0 bytes");
        assert_eq!(&t.dac[512..], &[7u8; 256], "entries 171.. keep what they had");
        // The general rule is 2*count + start bytes at byte offset start.
        t.dac = [7; 768];
        t.set_pal_area(&table, 4, 2);
        assert_eq!(&t.dac[4..12], &[9u8; 8], "2*2 + 4 bytes");
        assert_eq!(t.dac[3], 7);
        assert_eq!(t.dac[12], 7);
    }

    #[test]
    fn the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration() {
        let mut t = Techno::new();
        t.palanimc = 0;
        t.patdir = -3;
        t.palfader = 0;
        for _ in 0..101 {
            t.first_pass_iteration(0);
        }
        // 2*101 = 202: the cyan entry pal0[0] = (0,30,40) scaled by 202/256;
        // palanimc 9 puts it at entry 5 (the 8-triple window starting at
        // triple 3 ends on pal0's second cyan at triple 8).
        assert_eq!(t.palfader, 202);
        assert_eq!(t.palanimc, 9);
        assert_eq!(&t.dac[5 * 3..5 * 3 + 3], &[0u8, (30 * 202 / 256) as u8, (40 * 202 / 256) as u8]);
        assert!(t.dac[..15].iter().all(|v| *v == 0), "every other entry is black");
        // The second 8-entry window has the same cyan at entry 13; the
        // picture uses only indices 0..7, so entries 8..15 are invisible.
        assert!(t.dac[24..39].iter().all(|v| *v == 0));
        assert_eq!(&t.dac[39..42], &[0, 30 * 202 / 256, 40 * 202 / 256].map(|v: i32| v as u8));
        assert!(t.dac[42..48].iter().all(|v| *v == 0));
        for _ in 101..201 {
            t.first_pass_iteration(0);
        }
        assert_eq!(t.palfader, 402);
        // 402 - 256 = 146 added to every channel, clamped to 63: every entry
        // is white whatever the picture's index.
        assert!(
            t.dac[..48].iter().all(|v| *v == 63),
            "the fade is white at iteration 201"
        );
        for _ in 201..256 {
            t.first_pass_iteration(0);
        }
        assert_eq!(t.palfader, 512, "capped at 512");
    }

    #[test]
    fn the_interference_picture_is_the_circles_in_the_reference_window() {
        let mut t = Techno::new();
        t.circle2 = std::fs::read("../../assets/TECHNO/CIRCLE2.EGA").unwrap();
        t.circle = std::fs::read("../../assets/DDSTARS/PIC.EGA").unwrap();
        t.reset_video_for_tests();
        t.init_interference();
        // The reference's 4900 is the doubled picture at row 100, pixel 160:
        // the first pass's scrnpos is 80*100 + 160/8 and its pan 0.
        let mut surface = vec![0u8; 640 * 400];
        t.compose_ega(&mut surface);
        // The frame has exactly two colours: index 5 and index 0.
        let window: Vec<u8> = (0..200)
            .flat_map(|y| surface[y * 640..y * 640 + 320].to_vec())
            .collect();
        // The window's indices are the picture's planes, 0..7; the first
        // pass's palette makes exactly entry 5 cyan, so the reference's RGB
        // is two colours and the cyan count is the index-5 count.
        assert!(window.iter().all(|v| *v < 8), "the picture is three planes");
        assert_eq!(
            window.iter().filter(|v| **v == 5).count(),
            7888,
            "the reference's cyan pixel count"
        );
    }

    #[test]
    fn the_second_pass_circle_walks_one_eighty_byte_row_a_row() {
        let mut t = Techno::new();
        t.reset_video_for_tests();
        t.p2_scrnpos = 0;
        t.p2_scrnposl = 0;
        t.p2_overx = 0;
        t.p2_overya = 0;
        t.p2_sinuspower = 0;
        t.p2_sinurot = 0;
        // `power0`'s row 0 is all zeros, so every row's `ax` is 0 and the
        // window is always `circles[7]`. Make each source byte carry its
        // own row: the `push si` / `pop si` and `add si,80` of
        // `KOEA.ASM:790,812-814` then show as the drawn row.
        t.circles[7] = (0..32_768usize).map(|i| (i / 80) as u8).collect();
        t.interference_draw();
        for row in 0..200usize {
            for k in 0..44usize {
                assert_eq!(
                    t.planes[3][row * 80 + k],
                    row as u8,
                    "row {row} byte {k} reads 80 source bytes further down"
                );
            }
        }
    }

    #[test]
    fn doit3s_pan_is_the_low_three_bits_of_xpos() {
        // `KOE.C:528` sets ATC 13h to `xpos & 7` on the 16-colour screen; the
        // `(xpos & 3)*2` form belongs to the troll's 256-colour screen and
        // loses bit 2 (xpos 4 would pan 0 instead of 4).
        for xpos in [0i32, 1, 4, 5, 8, 13, 320] {
            let mut t = Techno::new();
            t.xpos = xpos;
            t.doit3_post();
            assert_eq!(t.pan, (xpos & 7) as u8, "xpos {xpos}");
        }
    }

    #[test]
    fn rotate1s_last_sixty_four_bytes_are_untouched() {
        let src: Vec<u8> = (0..32_000usize).map(|i| (i * 37 % 251) as u8).collect();
        let mut dst = vec![0xAAu8; 32_000];
        rotate1(&src, &mut dst);
        let mut carry = 0u8;
        for i in 0..32_000 - 64 {
            let value = src[i];
            assert_eq!(dst[i], (value >> 1) | (carry << 7), "byte {i}");
            carry = value & 1;
        }
        assert!(
            dst[32_000 - 64..].iter().all(|byte| *byte == 0xAA),
            "the source's loop stops 64 bytes short"
        );
    }

    #[test]
    fn flash_minus_two_uploads_nothing() {
        let mut t = Techno::new();
        t.dac = [7u8; 768];
        t.flash_pal = [11i16; 48];
        t.flash_apply(-2);
        assert_eq!(t.dac, [7u8; 768], "flash(-2) uploads nothing");
        assert_eq!(t.flash_pal, [11i16; 48], "and captures nothing");
    }

    #[test]
    fn the_second_pass_reads_the_sine_as_a_byte_offset() {
        let mut t = Techno::new();
        t.reset_video_for_tests();
        t.p2_scrnpos = 0;
        t.p2_scrnposl = 0;
        t.p2_overx = 0;
        t.p2_overya = 0;
        t.p2_sinuspower = 1;
        t.p2_sinurot = 0;
        for (k, circle) in t.circles.iter_mut().enumerate() {
            circle.fill(0x10 + k as u8);
        }
        t.interference_draw();
        let mut selected = std::collections::BTreeSet::new();
        for row in 0..200usize {
            // `sinurot += 14`, then `bp += 18` a row (`KOEA.ASM:785,791`).
            let bp = (14 + 18 * (row + 1)) & 2047;
            // The cursor is a byte offset; the word is `bp/2`.
            let mut bx = (SIN_TABLE[bp / 2] as i32) >> 3;
            bx = (bx & 0xFF) | (1 << 8);
            let ax = i32::from(t.power0[bx as usize]);
            let which = (7 - (ax & 7)) as usize;
            selected.insert(which);
            let si = (row as i32 * 80 + (ax >> 3)) as u16 as usize;
            for k in 0..44usize {
                assert_eq!(
                    t.planes[3][row * 80 + k],
                    t.circles[which][si + k],
                    "row {row} byte {k} is circle {which} at {si}"
                );
            }
        }
        assert!(
            selected.len() > 1,
            "sinuspower 1 makes the byte cursor select more than one circle"
        );
    }

    #[test]
    fn lineblit_writes_pixel_x_to_plane_x_mod_four() {
        let mut t = Techno::new();
        t.reset_video_for_tests();
        let mut row = vec![0u8; 320];
        for (x, value) in row.iter_mut().enumerate() {
            *value = (x * 7 % 251) as u8;
        }
        t.lineblit(&row, 3);
        for (x, &pixel) in row.iter().enumerate() {
            let plane = x & 3;
            let byte = x >> 2;
            let at = 80 + 3 * 160 + byte;
            assert_eq!(
                t.planes[plane][at], pixel,
                "pixel {x} is plane {plane} byte {byte}"
            );
        }
    }

    #[test]
    fn the_box_renderer_fills_the_clipped_polygon() {
        let mut t = Techno::new();
        // `cap3` clips the closing edge first and its result is the first
        // edge, so a box clips to its own corners (no pseudo-point).
        t.asmbox([(10, 10), (30, 10), (30, 30), (10, 30)]);
        t.blit(0, 1, PLANE_ROW);
        let bit = |x: usize, y: usize| -> u8 {
            (t.planes[0][y * 40 + x / 8] >> (7 - (x & 7))) & 1
        };
        // The even-odd fill of the rectangle: rows 10..30, columns 10..30.
        // `drawline` skips its horizontal edges and drops its last row, so
        // the fill is bounded by the two vertical edges and the closing
        // diagonal from the pseudo-free polygon.
        for y in 10..30usize {
            for x in 10..30usize {
                assert_eq!(bit(x, y), 1, "the interior pixel ({x},{y})");
            }
        }
        assert_eq!(bit(5, 15), 0, "outside");
        assert_eq!(bit(30, 15), 0, "the right edge is the source's half-open fill");
        // The rectangle's own outline is in the vbuf.
        assert!((t.vbuf[10 * 40 + 30 / 8] & (0x80 >> (30 & 7))) != 0);
    }

    #[test]
    fn the_troll_rle_is_the_origin_readers_rows() {
        let bytes = std::fs::read("../../assets/TECHNO/TROLL.UP").unwrap();
        let rows = readp_rows(&bytes);
        assert_eq!(rows.len(), 400);
        assert!(rows.iter().all(|row| row.len() == 320));
        let image = sr_assets::load_path(std::path::Path::new("../../assets/TECHNO/TROLL.UP"))
            .expect("the Origin reader loads it");
        assert_eq!(image.width(), 320);
        assert_eq!(image.height(), 400);
        for (y, row) in rows.iter().enumerate() {
            assert_eq!(
                row.as_slice(),
                &image.pixels()[y * 320..(y + 1) * 320],
                "row {y}"
            );
        }
    }

    #[test]
    fn the_troll_palette_is_the_files_own_six_bit_palette() {
        let bytes = std::fs::read("../../assets/TECHNO/TROLL.UP").unwrap();
        let image = sr_assets::load_path(std::path::Path::new("../../assets/TECHNO/TROLL.UP"))
            .expect("the Origin reader loads it");
        let file = &bytes[16..16 + 256 * 3];
        for entry in 0..=255u8 {
            let at = usize::from(entry) * 3;
            assert_eq!(
                image.palette.entry(entry),
                [file[at], file[at + 1], file[at + 2]],
                "entry {entry}"
            );
        }
    }

    #[test]
    fn the_entry_pre_roll_lands_the_reference_position_at_tick_zero() {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;
        use sr_engine::music::Music;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(18);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        let position = music.position();
        assert_eq!((position.order, position.row), (18, 60));
        assert_eq!(music.musplus(), -4, "the first gate (`<-4`) passes");
        // The engine's tick-0 poll leaves the published row where it was.
        music.poll();
        let position = music.position();
        assert_eq!((position.order, position.row), (18, 60));
        assert_eq!(music.musplus(), -4);
    }

    #[test]
    fn the_troll_fade_tables_add_the_sources_steps() {
        let mut palette = [0u8; 768];
        for (i, v) in palette.iter_mut().enumerate() {
            *v = (i % 61) as u8;
        }
        let fade = troll_fade(&palette);
        for y in 0..16usize {
            let step = 45 - 3 * y as i32;
            for a in 0..768usize {
                let want = (palette[a] as i32 + step).min(63) as u8;
                assert_eq!(fade[y * 768 + a], want, "y={y} a={a}");
            }
        }
        assert_eq!(fade[0], (palette[0] as i32 + 45).min(63) as u8);
        assert_eq!(fade[15 * 768], palette[0], "the last table is the palette");
    }
}
