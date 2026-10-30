//! JPLOGO — the jelly logo.
//!
//! `JPLOGO/JP.C` (227 lines) over `JPLOGO/ASM.ASM` (`inittwk`, `lineblit`,
//! `setpalarea`, `linezoom`), the generated `JPLOGO/ZOOM.INC` (16,837 lines,
//! one unrolled routine per even scale 140..244) and `JPLOGO/READP.C`, with the
//! linked `ICEKNGDM.UP` (`_PIC.OBK`) and JPLOGO's **own** `SIN1024.INC`.
//!
//! # The screen
//!
//! `mov ax,13h / int 10h` (`JP.C:173-174`) then `inittwk` (`ASM.ASM:36-74`):
//! CRTC 0x09 = 0 (400 lines, no doubling), 0x14 = 0, 0x17 = 0xE3, chain-4 off,
//! map mask 0x0F, and a 64 KB `rep stosw` which the unchained handler writes
//! as 64 K dwords — the whole 256 KB. It never writes CRTC 0x13, so the offset
//! stays mode 13h's 40 words = **80 plane bytes a row**. In the emulator's
//! unchained model a CPU byte address is four plane bytes, and the display
//! reads them as four consecutive pixels, so the picture is a flat 320×400
//! surface at 320 bytes a row and `scrolly(y)` (`JP.C:30-45`) starts the
//! display at plane byte `y*80` — pixel origin `y*320`.
//!
//! # The three phases
//!
//! 1. **Init** (`JP.C:107-210`): the spring tables (`:117-171`), `inittwk`,
//!    the DAC writes, `readp` over the picture (400 rows cropped to x=70..253
//!    with a synthetic 65 border), `setpalarea(palette,0,256)` — whose 512-byte
//!    upload is [`palette_upload`] — the source 0→64 remap, and the one full
//!    400-row draw at scale 184.
//! 2. **The entry** (`:204-220`): `scrolly(400)`, one 400-row draw, then the
//!    83-step loop that walks the start address from 399 to 0, one step a
//!    vblank. The picture reveals top-down into the start address.
//! 3. **`doit`** (`:67-105`): the `dis_musplus()<4` wait (which at this point
//!    of MUSIC1 is the order-98 row-4 crossing), `while(frame<700)`, the
//!    freeze after 511, `xsc`, the per-row `b` and `a`, the palimpsest guard
//!    (`lasty`/`lasts`) and `linezoom`.
//!
//! # The tick model, and the measured mapping
//!
//! `dis_waitb()` returns the number of copper frames since the previous call
//! (`DIS/DISINT.ASM:161-183`), so the source's counters are vblank counters.
//! The port runs one engine tick a displayed frame: one entry step or one
//! `doit` iteration a tick. The `doit` counter [`Jplogo::frame`] advances by
//! **one** a tick — the reference's own cadence at the keyframes (the fixed
//! cost model is in `docs/fidelity/APPROXIMATIONS.md`). A `--part jplogo` run
//! is part-numbered (`index_origin: "part"`, no anchor); the mapping the
//! reference supports is
//!
//! ```text
//! reference  rendered tick
//! 17100      35            (the sweep's entry, start 309)
//! 17300      235
//! 17500      435
//! 17700      635
//! ```
//!
//! i.e. tick `t` is boot frame `17065 + t`. The entry's tick 0 is the
//! `scrolly(400)` screen; ticks 1..83 are the 83 steps; tick 84 is the
//! `dis_waitb()` between the loop and `doit()`; tick 85 begins the `doit`
//! iterations with the counter at 1.
//!
//! # The music
//!
//! `shell::music_for_part("jplogo")` seeks **MUSIC1 order 96**, the order the
//! `second3d` trace holds across the whole start window (17000..17100, rows
//! 50..62, pattern 71). [`Jplogo::MUSIC_PRE_ROLL`] pre-polls 483 frames, the
//! count that puts the player's row-58 boundary at tick -5 — boot 17060, where
//! the trace's row 58 begins — so the published position is **96:58** at tick
//! 0 and every later row boundary lines up: 96:59 at tick 3, 98:0 at 44,
//! 98:4 at 76, 98:5 at 84, 98:23 at 235, 98:48 at 435 and 98:49 at 635. A
//! mid-chain start polls nothing (`PartCtx::music_fresh`).
//!
//! # The preserved bugs
//!
//! - `_setpalarea`'s 512-byte upload: `mov cx,count; shl cx,1; add cx,start;
//!   rep outsb` (`ASM.ASM:104-126`) writes 512 bytes of the 768-byte palette —
//!   entries 0..169 whole, entry 170's red and green, nothing after. The
//!   picture uses only indices 1..58, 64 and 65, so it is invisible here and
//!   mandatory all the same.
//! - JPLOGO's own `SIN1024.INC`: index 256 is 255 and 768 is -255, not 256 and
//!   -256; [`SIN1024`] is this file, not another part's copy.
//! - The crop at x=70 and the synthetic border byte 65 at index 184.
//! - The source 0→64 remap with palette entry 64 blacked.
//! - The interpolation's out-of-bounds read at `framey1[200]`/`framey2[200]`
//!   (`JP.C:169-170`); the four entries are never read (`doit` uses ≤511).
//! - The freeze's unused `c=400` (`JP.C:78`).
//! - The palimpsest redraw guard (`JP.C:96-99`): a row whose `(b,a)` repeat
//!   keeps whatever an earlier state drew there.
//! - `scrolly` writes the low byte to 0x0D first, then the high byte to 0x0C;
//!   the port's [`View::set_origin`] is one 16-bit display start.
//!
//! [`View::set_origin`]: sr_engine::view::View::set_origin

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{asset, load_image, mode_view};

/// The part's registry name: what `--part` accepts.
const NAME: &str = "jplogo";

/// The linked picture (`_PIC.OBK`'s payload).
const PICTURE_PATH: &str = "assets/JPLOGO/ICEKNGDM.UP";

// --------------------------------------------------------------- screens --

/// The picture's width: `hdr->wid`.
const PICTURE_WIDTH: usize = 320;
/// The picture's height, and the displayed 400 lines.
const PICTURE_HEIGHT: usize = 400;
/// The displayed width.
const SCREEN_WIDTH: u16 = 320;
/// The displayed height.
const SCREEN_HEIGHT: u16 = 400;
/// 320 pixels a row, packed: mode 13h's CRTC offset 40 words = 80 plane bytes.
const LINE_LEN: u16 = 320;

// ---------------------------------------------------------------- picture --

/// `memcpy(row[y],rowbuf+70,184)` (`JP.C:194`).
const CROP_LEFT: usize = 70;
/// The crop's width.
const CROP_WIDTH: usize = 184;
/// `row[y][184]=65` (`:195`) — the synthetic white border the zoom samples as
/// source index 184.
const BORDER_INDEX: u8 = 65;
/// A source row and its border byte.
const ROW_BYTES: usize = CROP_WIDTH + 1;
/// The index the 0→64 remap writes (`:200`), whose palette entry `:188-190`
/// blacks.
const BLACK_INDEX: u8 = 64;

// ---------------------------------------------------------------- palette --

/// `setpalarea(palette,0,256)`'s count: `2*count + start` = **512** bytes of
/// the 768-byte array (`ASM.ASM:104-126`).
const PALETTE_UPLOAD_BYTES: usize = 512;
/// Entries fully written by those 512 bytes: `512 / 3 = 170` and 2 bytes over.
const PALETTE_ENTRIES_WHOLE: usize = PALETTE_UPLOAD_BYTES / 3;
/// The entry the upload stops inside.
const PALETTE_PARTIAL: usize = PALETTE_ENTRIES_WHOLE;

// ------------------------------------------------------------------ entry --

/// `a=64; y=400*64; while(y>0){ y-=a; a+=6; ... }` (`JP.C:211-219`).
const ENTRY_STEPS: usize = 83;
/// The tick the entry's step `i` first shows at (tick 0 is `scrolly(400)`).
const ENTRY_FIRST_TICK: u32 = 1;
/// `dis_waitb()` between the entry loop and `doit()` (`:221`).
const EXTRA_WAIT_TICK: u32 = ENTRY_FIRST_TICK + ENTRY_STEPS as u32;
/// The first tick `doit()`'s loop body runs on; its counter reads 1 there.
const DOIT_FIRST_TICK: u32 = EXTRA_WAIT_TICK + 1;

// -----------------------------------------------------------------  doit --

/// `while(!dis_exit() && frame<700)` (`JP.C:72`).
const FRAME_CAP: u32 = 700;
/// `if(frame>511) c=400; else { y1=...; y2=...; }` (`:78-83`).
const FREEZE_AFTER: u32 = 511;
/// The entry pre-roll: the polls that put the player at the reference's
/// position for tick 0 (96:58) with its row boundaries on the trace's.
const MUSIC_PRE_ROLL: u32 = 483;

// ----------------------------------------------------------------- tables --

/// `JPLOGO/SIN1024.INC`, 1,024 signed words: indices 256 and 768 are ±255, not
/// the common copy's ±256.
#[rustfmt::skip]
const SIN1024: [i16; 1024] = [
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
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254,
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
    -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -254,
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

// ------------------------------------------------------------- the spring --

/// A 16-bit C `int` assigned from `x`.
fn wrap16(x: i32) -> i32 {
    i32::from(x as i16)
}

/// `JP.C:117-163`: the two 200-entry spring tables, in the source's 16-bit
/// `int` arithmetic. `a` is 200 on entry — left there by the row-pointer
/// loops (`:114-115`) — which is what `la=a` (`:140`) reads on the first pass.
fn spring_tables() -> ([i16; 200], [i16; 200]) {
    let mut fy1 = [0i16; 200];
    let mut fy2 = [0i16; 200];
    let mut y1 = 0i32;
    let mut y1a = 500i32;
    let mut y2 = 399 * 16;
    let mut y2a = 500i32;
    let mut mika = 1i32;
    let mut halt = false;
    // `for(a=0;a<200;a++)` at `:114-115` leaves `a` at 200, which is what
    // `la=a` reads on the first pass.
    let mut a = 200i32;
    for frame in 0..200 {
        if !halt {
            y1 = wrap16(y1 + y1a);
            y2 = wrap16(y2 + y2a);

            y2a = wrap16(y2a + 16);
            if y2 > 400 * 16 {
                y2 = wrap16(y2 - y2a);
                y2a = wrap16(-y2a * mika / 8);
                if mika < 4 {
                    mika += 3;
                }
            }

            y1a = wrap16(y1a + 16);

            let la = a;
            a = wrap16((y2 - y1) - 400 * 16);
            if (a & 0x8000) ^ (la & 0x8000) != 0 {
                y1a = wrap16(y1a * 7 / 8);
            }
            y1a = wrap16(y1a + a / 8);
            y2a = wrap16(y2a - a / 8);
        }

        if frame > 90 {
            if y2 >= 399 * 16 {
                y2 = 400 * 16;
                halt = true;
            } else {
                y2a = 8;
            }
            y1 = wrap16(y2 - 400 * 16);
        }

        fy1[frame] = y1 as i16;
        fy2[frame] = y2 as i16;
    }
    (fy1, fy2)
}

/// `JP.C:164-171`: the 4× interpolation to 800 entries. `a/4` is at most 199,
/// so `framey1[b+1]`/`framey2[b+1]` read one past each array for `a >= 796`.
/// The C globals are declared in that order (`:23-26`), so the read is the
/// next array's first element; the four entries are never read by `doit`
/// (`frame <= 511`).
fn interpolate_800(fy1: &[i16; 200], fy2: &[i16; 200]) -> ([i32; 800], [i32; 800]) {
    let mut t1 = [0i32; 800];
    let mut t2 = [0i32; 800];
    for a in 0..800 {
        let b = a / 4;
        let c = (a & 3) as i32;
        let d = 3 - c;
        // `framey1[b+1]` past the end is `framey2[0]` — the next global in
        // the source's declaration order (`:23-24`) — and `framey2[b+1]`
        // past the end is `framey1t[0]`.
        let next1 = if b + 1 < 200 {
            i32::from(fy1[b + 1])
        } else {
            i32::from(fy2[0])
        };
        t1[a] = (i32::from(fy1[b]) * d + next1 * c) / 3;
        let next2 = if b + 1 < 200 {
            i32::from(fy2[b + 1])
        } else {
            t1[0]
        };
        t2[a] = (i32::from(fy2[b]) * d + next2 * c) / 3;
    }
    (t1, t2)
}

/// `JP.C:211-219`: the 83 start-address rows the entry loop walks.
fn entry_scrolls() -> Vec<u16> {
    let mut y = 400 * 64i32;
    let mut a = 64i32;
    let mut scrolls = Vec::with_capacity(ENTRY_STEPS);
    while y > 0 {
        y -= a;
        a += 6;
        if y < 0 {
            y = 0;
        }
        scrolls.push((y / 64) as u16);
    }
    scrolls
}

// --------------------------------------------------------------- the zoom --

/// One `linezoom(vram + y*80, src, a)` (`ASM.ASM:128-153` over `ZOOM.INC`).
///
/// The generated routines write, for every pixel in the fixed window
/// `[32,288)` and no pixel outside it: the source byte
/// `((x-le)*185 + c/2)/c` for `x` in `[le,ri]`, black otherwise. `a` is
/// clamped to 318 with an **unsigned** compare and rounded down to even
/// (`:140-143`); the resulting table index then selects a **label**:
/// 0 is `@zoom0`, 2..138 is `@zoom140`, 140..244 is that scale and 246..318
/// is `@zoom244` (`DOL.C:31-38`). The routine's geometry is the label's, not
/// the index's. `src` `None` is `@zoom0`, which writes black.
///
/// The part's own scales stay well inside the generated range:
/// `the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside`
/// measures the reachable set over every `doit` frame as [164, 224], so the
/// two clamps are unreachable here and carried anyway.
fn zoom_row(pixels: &mut [u8], y: usize, src: Option<&[u8; ROW_BYTES]>, scale: i32) {
    // `_linezoom`'s `cmp bx,318 / jbe` is an **unsigned** compare, and
    // `and bx,not 1` follows it (`ASM.ASM:140-143`).
    let mut index = scale as u16 as i32;
    if (index as u16) > 318 {
        index = 318;
    }
    index &= !1;
    // `zoomt`'s dispatch (`DOL.C:31-38`).
    let a = if index == 0 { 0 } else { index.clamp(140, 244) };

    let base = y * SCREEN_WIDTH as usize;
    let Some(src) = src else {
        // `@zoom0`: `mov es:[di+8],ax` through `es:[di+70]`, with the map
        // mask 0x0F — pixels 32..287 in every plane, and the fixed margins
        // `[0,32)`/`[288,320)` untouched.
        for x in 32..288 {
            pixels[base + x] = 0;
        }
        return;
    };
    if a == 0 {
        // `@zoom0` selected with a source still draws black.
        for x in 32..288 {
            pixels[base + x] = 0;
        }
        return;
    }

    let le = 160 - a / 2;
    let ri = 160 + a / 2;
    let c = ri - le + 1;
    for x in 32..288usize {
        let xi = x as i32;
        pixels[base + x] = if xi < le || xi > ri {
            0
        } else {
            src[(((xi - le) * 185 + c / 2) / c) as usize]
        };
    }
}

// ------------------------------------------------------------ the palette --

/// The DAC upload `setpalarea(palette,0,256)` (`ASM.ASM:104-126`) performs on
/// the palette state `dac`: `2*count + start` = 512 bytes, no more.
///
/// `init` passes [`Palette::new`] — all black — because `inittwk`
/// (`ASM.ASM:31-38`) writes 768 zero bytes to the DAC data port before the
/// upload, so the source's post-init DAC is the file's entries 0..169, entry
/// 170's red and green over a **zero** blue, and a black 171..255. The
/// entries past 169 are inherited from that clear, not from whatever part ran
/// before this one.
fn palette_upload(file: &Palette, dac: Palette) -> Palette {
    let mut out = dac;
    for entry in 0..PALETTE_ENTRIES_WHOLE {
        let blacked = if entry == usize::from(BLACK_INDEX) {
            [0, 0, 0]
        } else {
            file.entry(entry as u8)
        };
        out.set_entry(entry as u8, blacked);
    }
    // 512 bytes stop after entry 170's green; its blue is the DAC's old one.
    let old_blue = dac.entry(PALETTE_PARTIAL as u8)[2];
    let partial = file.entry(PALETTE_PARTIAL as u8);
    out.set_entry(PALETTE_PARTIAL as u8, [partial[0], partial[1], old_blue]);
    out
}

// --------------------------------------------------------------- the part --

/// JPLOGO's state: the decoded rows, the two interpolated tables, the entry's
/// start addresses, the palimpsest guard and the `doit` counter.
pub struct Jplogo {
    rows: Vec<[u8; ROW_BYTES]>,
    t1: [i32; 800],
    t2: [i32; 800],
    scrolls: Vec<u16>,
    lasty: [i32; PICTURE_HEIGHT],
    lasts: [i32; PICTURE_HEIGHT],
    /// `doit`'s `frame`.
    frame: u32,
    /// `doit`'s `y1`/`y2`, carried between iterations.
    y1: i32,
    y2: i32,
    gate_open: bool,
    /// `doit`'s `*shiftstatus & 16` (`JP.C:74-76`): Scroll Lock shows the
    /// border. The engine has no keyboard state, so nothing in a run ever
    /// sets it; the branch is kept and unit-tested through this field.
    scroll_lock: bool,
    done: bool,
}

impl Jplogo {
    /// A fresh part: `init` reads `assets/JPLOGO/ICEKNGDM.UP`.
    pub fn new() -> Jplogo {
        Jplogo {
            rows: Vec::new(),
            t1: [0; 800],
            t2: [0; 800],
            scrolls: Vec::new(),
            lasty: [-1; PICTURE_HEIGHT],
            lasts: [-1; PICTURE_HEIGHT],
            frame: 0,
            y1: 0,
            y2: 0,
            gate_open: false,
            scroll_lock: false,
            done: false,
        }
    }

    /// The 400 rows `readp` decodes and `:191-196` crops: 184 bytes from x=70,
    /// 0→64, and the synthetic 65 border.
    fn read_rows(image: &sr_assets::Image) -> Vec<[u8; ROW_BYTES]> {
        let mut rows = Vec::with_capacity(PICTURE_HEIGHT);
        for y in 0..PICTURE_HEIGHT {
            let source = image.surface.row(y);
            let mut row = [0u8; ROW_BYTES];
            row[..CROP_WIDTH].copy_from_slice(&source[CROP_LEFT..CROP_LEFT + CROP_WIDTH]);
            for byte in &mut row[..CROP_WIDTH] {
                if *byte == 0 {
                    *byte = BLACK_INDEX;
                }
            }
            row[CROP_WIDTH] = BORDER_INDEX;
            rows.push(row);
        }
        rows
    }

    /// One `doit` body after the counter and `y1`/`y2` are settled: every row
    /// in `[y1,y2)` is redrawn only when its `(b,a)` differ from `lasty`/
    /// `lasts`; every row outside is cleared (`JP.C:85-103`).
    fn draw_doit_frame(&mut self, pixels: &mut [u8]) {
        let xsc = (400 - (self.y2 - self.y1)) / 8;
        for y in 0..PICTURE_HEIGHT {
            let yi = y as i32;
            if yi < self.y1 || yi >= self.y2 {
                zoom_row(pixels, y, None, 0);
            } else {
                let b = (yi - self.y1) * 400 / (self.y2 - self.y1);
                let a = 184
                    + (i32::from(SIN1024[b as usize * 32 / 25]) * xsc + 32) / 64;
                let a = a & !1;
                if self.lasty[y] != b || self.lasts[y] != a {
                    zoom_row(pixels, y, Some(&self.rows[b as usize]), a);
                    self.lasty[y] = b;
                    self.lasts[y] = a;
                }
            }
        }
    }
}

impl Default for Jplogo {
    fn default() -> Self {
        Jplogo::new()
    }
}

impl Part for Jplogo {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part opens with a music gate (`JP.C:71` \
             `while(!dis_exit() && dis_musplus()<4)`) and the run has no music",
        )?;

        let path = asset(PICTURE_PATH);
        let image = load_image(NAME, PICTURE_PATH)?;
        if (image.width() as usize, image.height() as usize)
            != (PICTURE_WIDTH, PICTURE_HEIGHT)
        {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{path} is {}x{}; the picture's rows are {}x{PICTURE_HEIGHT}",
                    image.width(),
                    image.height(),
                    PICTURE_WIDTH
                ),
            });
        }

        // `readp(palette,-1,pic)` / `:188-196`'s palette and rows. `inittwk`
        // has already written 768 zeros to the DAC (`ASM.ASM:31-38`), so the
        // 512-byte upload builds on black, not on the inherited palette.
        self.rows = Self::read_rows(&image);
        let palette = palette_upload(&image.palette, Palette::new());
        ctx.set_palette(palette);

        // `:117-171`'s tables, `:211-219`'s entry.
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        self.t1 = t1;
        self.t2 = t2;
        self.scrolls = entry_scrolls();
        self.lasty = [-1; PICTURE_HEIGHT];
        self.lasts = [-1; PICTURE_HEIGHT];
        self.frame = 0;
        self.y1 = 0;
        self.y2 = 0;
        self.gate_open = false;
        self.scroll_lock = false;
        self.done = false;

        // `inittwk` (`ASM.ASM:36-74`): CRTC 0x09 = 0 (400 lines, one a row),
        // no offset register written — 80 plane bytes a row = 320 pixels — and
        // AC 0x11 = 0xFF. `scrolly(400)` is the entry's first start address.
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(LINE_LEN), Some(255));
        ctx.view.set_origin(u32::from(400u16) * u32::from(LINE_LEN));

        // `inittwk`'s 64 KB `rep stosw` reaches all 256 KB through the
        // unchained handler; the screen and the start address's second half
        // both begin black.
        ctx.surface.pixels_mut().fill(0);

        // `:207-210`: one full 400-row draw at scale 184.
        let pixels = ctx.surface.pixels_mut();
        for y in 0..PICTURE_HEIGHT {
            zoom_row(pixels, y, Some(&self.rows[y]), 184);
        }

        // The entry pre-roll: only a run that began here catches the player up.
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
        }
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.done {
            return PartStatus::Finished;
        }
        if ctx.exit_requested {
            self.done = true;
            return PartStatus::Finished;
        }

        // The entry's tick schedule: tick 0 is the `scrolly(400)` screen
        // `init` left; ticks 1..83 are the loop's 83 steps (`JP.C:211-219`);
        // tick 84 is the `dis_waitb()` at `:221`.
        let tick = ctx.frame;
        if tick < DOIT_FIRST_TICK {
            if (ENTRY_FIRST_TICK..=EXTRA_WAIT_TICK).contains(&tick) {
                let step = (tick - ENTRY_FIRST_TICK) as usize;
                if step < ENTRY_STEPS {
                    ctx.view
                        .set_origin(u32::from(self.scrolls[step]) * u32::from(LINE_LEN));
                }
            }
            return PartStatus::Continuing;
        }

        // `doit`'s gate (`JP.C:71`): wait for `musplus >= 4`. The wait is a
        // spin in the source; here it costs ticks, not `doit` frames.
        if !self.gate_open {
            let musplus = ctx.musplus();
            if musplus < 4 {
                return PartStatus::Continuing;
            }
            self.gate_open = true;
        }

        if self.frame >= FRAME_CAP {
            self.done = true;
            return PartStatus::Finished;
        }

        // `:74-76`: Scroll Lock paints the border around the wait and the
        // drawing. No input path reaches this in a run.
        if self.scroll_lock {
            ctx.view.border = Some(0);
        }

        // `frame += c` (`:77`) with `c` the vblank count: the model is one a
        // tick (`docs/fidelity/APPROXIMATIONS.md`). `if(frame>511) c=400;`
        // (`:78`) never assigns `y1`/`y2` and `c` is not read again.
        self.frame += 1;
        if self.frame <= FREEZE_AFTER {
            self.y1 = self.t1[self.frame as usize] / 16;
            self.y2 = self.t2[self.frame as usize] / 16;
        }

        self.draw_doit_frame(ctx.surface.pixels_mut());
        if self.scroll_lock {
            ctx.view.border = Some(127);
        }
        PartStatus::Continuing
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::read_asset;
    use crate::audio::PlayerMusic;
    use crate::parts::test_rig::{
        differing_pixels, reference, rgb_of, trace_position, Reading, Rig, ScriptedMusic,
    };
    use sr_engine::music::Music;

    // ------------------------------------------------------- the source ----

    fn reference_tables() -> ([i16; 200], [i16; 200], [i32; 800], [i32; 800]) {
        let mut fy1 = [0i16; 200];
        let mut fy2 = [0i16; 200];
        let mut y1 = 0i32;
        let mut y1a = 500i32;
        let mut y2 = 399 * 16;
        let mut y2a = 500i32;
        let mut mika = 1i32;
        let mut halt = false;
        // `for(a=0;a<200;a++)` leaves `a` at 200 before the spring loop.
        let mut a = 200i32;
        for frame in 0..200 {
            if !halt {
                y1 = wrap16(y1 + y1a);
                y2 = wrap16(y2 + y2a);
                y2a = wrap16(y2a + 16);
                if y2 > 400 * 16 {
                    y2 = wrap16(y2 - y2a);
                    y2a = wrap16(-y2a * mika / 8);
                    if mika < 4 {
                        mika += 3;
                    }
                }
                y1a = wrap16(y1a + 16);
                let la = a;
                a = wrap16((y2 - y1) - 400 * 16);
                if (a & 0x8000) ^ (la & 0x8000) != 0 {
                    y1a = wrap16(y1a * 7 / 8);
                }
                y1a = wrap16(y1a + a / 8);
                y2a = wrap16(y2a - a / 8);
            }
            if frame > 90 {
                if y2 >= 399 * 16 {
                    y2 = 400 * 16;
                    halt = true;
                } else {
                    y2a = 8;
                }
                y1 = wrap16(y2 - 400 * 16);
            }
            fy1[frame] = y1 as i16;
            fy2[frame] = y2 as i16;
        }
        let mut t1 = [0i32; 800];
        let mut t2 = [0i32; 800];
        for a in 0..800 {
            let b = a / 4;
            let c = (a & 3) as i32;
            let d = 3 - c;
            // `framey1[200]` is `framey2[0]` and `framey2[200]` is
            // `framey1t[0]` under the source's declaration order.
            let n1 = if b + 1 < 200 { i32::from(fy1[b + 1]) } else { i32::from(fy2[0]) };
            t1[a] = (i32::from(fy1[b]) * d + n1 * c) / 3;
            let n2 = if b + 1 < 200 { i32::from(fy2[b + 1]) } else { t1[0] };
            t2[a] = (i32::from(fy2[b]) * d + n2 * c) / 3;
        }
        (fy1, fy2, t1, t2)
    }

    fn u16_at(bytes: &[u8], at: usize) -> usize {
        usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]))
    }

    /// `READP.C:30-63`'s RLE loop. A byte >= 0x80 is a run: `ah = b & 0x7f`,
    /// then `mov es:[di],al / inc di / dec ah / jnz` — 8-bit, so a 0x80
    /// opcode wraps on its first decrement and writes **256** copies.
    fn rle_decode(data: &[u8], out: &mut Vec<u8>) {
        let mut i = 0;
        while i < data.len() {
            let b = data[i];
            i += 1;
            if b >= 0x80 {
                let mut ah = b & 0x7f;
                let value = data[i];
                i += 1;
                loop {
                    out.push(value);
                    ah = ah.wrapping_sub(1);
                    if ah == 0 {
                        break;
                    }
                }
            } else {
                out.push(b);
            }
        }
    }

    /// `READP.C`'s row read, transliterated over the file's bytes.
    fn reference_readp_row(bytes: &[u8], row: i32) -> Vec<u8> {
        let hig = u16_at(bytes, 4);
        if row >= 0 && row as usize >= hig {
            return Vec::new();
        }
        let add = u16_at(bytes, 8);
        let mut src = add * 16;
        let mut left = row;
        while left > 0 {
            let len = u16_at(bytes, src);
            src += len + 2;
            left -= 1;
        }
        let count = u16_at(bytes, src);
        src += 2;
        let mut out = Vec::new();
        rle_decode(&bytes[src..src + count], &mut out);
        out
    }

    fn reference_palette(bytes: &[u8]) -> Vec<u8> {
        let cols = u16_at(bytes, 6);
        bytes[16..16 + cols * 3].to_vec()
    }

    fn asset() -> sr_assets::Image {
        load_image(NAME, PICTURE_PATH).expect("ICEKNGDM.UP")
    }

    fn patterned_palette() -> Palette {
        let mut pal = Palette::new();
        for index in 0..256u16 {
            pal.set_entry(
                index as u8,
                [(index % 7) as u8, (index % 11) as u8, (index % 13) as u8],
            );
        }
        pal
    }

    fn reference_zoom_row(y: usize, src: Option<&[u8; ROW_BYTES]>, scale: i32) -> Vec<u8> {
        let mut row = vec![0xAAu8; PICTURE_WIDTH];
        let mut index = scale as u16 as i32;
        if (index as u16) > 318 {
            index = 318;
        }
        index &= !1;
        let a = if index == 0 { 0 } else { index.clamp(140, 244) };
        let _ = y;
        let Some(src) = src else {
            row[32..288].fill(0);
            return row;
        };
        if a == 0 {
            row[32..288].fill(0);
            return row;
        }
        let le = 160 - a / 2;
        let ri = 160 + a / 2;
        let c = ri - le + 1;
        for (offset, pixel) in row[32..288].iter_mut().enumerate() {
            let xi = (offset + 32) as i32;
            *pixel = if xi < le || xi > ri {
                0
            } else {
                src[(((xi - le) * 185 + c / 2) / c) as usize]
            };
        }
        row
    }

    // ------------------------------------------------------ the tables ----

    #[test]
    fn the_sine_table_is_the_sources_own_1024_words() {
        let text = std::fs::read_to_string("../../upstream/SecondReality/JPLOGO/SIN1024.INC")
            .expect("SIN1024.INC");
        let mut words = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("dw") {
                for word in rest.split(',') {
                    let word = word.trim();
                    if !word.is_empty() {
                        words.push(word.parse::<i16>().unwrap());
                    }
                }
            }
        }
        assert_eq!(words.len(), 1024);
        assert_eq!(&words[..], &SIN1024[..]);
        // This copy, not the common one: indices 256 and 768 are already at
        // the peak.
        assert_eq!(SIN1024[256], 255);
        assert_eq!(SIN1024[768], -255);
    }

    #[test]
    fn the_spring_tables_are_the_sources_own_first_frames() {
        let (fy1, fy2) = spring_tables();
        assert_eq!(&fy1[..6], &[500, 885, 1172, 1323, 1317, 1153]);
        assert_eq!(&fy2[..6], &[6368, 6370, 6354, 6338, 6322, 6306]);
        assert!(fy1[95..].iter().all(|&v| v == 0), "framey1[95:]");
        assert!(fy2[95..].iter().all(|&v| v == 6400), "framey2[95:]");
        let (rfy1, rfy2, _, _) = reference_tables();
        assert_eq!(fy1, rfy1);
        assert_eq!(fy2, rfy2);
    }

    #[test]
    fn the_interpolation_is_the_sources_own_and_its_overrun_is_unreachable() {
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        assert_eq!(&t1[..6], &[500, 628, 756, 885, 885, 980]);
        assert_eq!(&t2[..6], &[6368, 6368, 6369, 6370, 6370, 6364]);
        assert_eq!(&t1[796..], &[0, 2122, 4245, 6368]);
        assert_eq!(&t2[796..], &[6400, 4433, 2466, 500]);
        let (_, _, rt1, rt2) = reference_tables();
        assert_eq!(t1, rt1);
        assert_eq!(t2, rt2);
        // The settle: the first counter whose two entries are the whole
        // screen.
        assert_eq!((t1[379] / 16, t2[379] / 16), (0, 400));
        // `doit` reads `framey1t[frame]` for `frame <= 511`; the four entries
        // the out-of-bounds read computes are never read.
        const { assert!(FREEZE_AFTER <= 511) };
    }

    #[test]
    fn the_entry_walks_eighty_three_start_rows_from_399_to_zero() {
        let scrolls = entry_scrolls();
        assert_eq!(scrolls.len(), ENTRY_STEPS);
        assert_eq!(scrolls[0], 399);
        assert_eq!(scrolls[1], 397);
        assert_eq!(scrolls[2], 396);
        assert_eq!(*scrolls.last().unwrap(), 0);
        assert!(scrolls.windows(2).all(|w| w[0] > w[1]), "strictly decreasing");
        // The source's own loop, written out.
        let mut y = 400 * 64i32;
        let mut a = 64i32;
        let mut expected = Vec::new();
        while y > 0 {
            y -= a;
            a += 6;
            if y < 0 {
                y = 0;
            }
            expected.push((y / 64) as u16);
        }
        assert_eq!(scrolls, expected);
    }

    // -------------------------------------------------------- the zoom ----

    #[test]
    fn the_zoom_window_is_the_source_bytes_at_each_scale() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        for scale in [140, 141, 142, 144, 184, 200, 244, 318, 320, -3] {
            for y in [0usize, 199, 399] {
                let mut mine = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
                zoom_row(&mut mine, y, Some(&rows[y]), scale);
                let theirs = reference_zoom_row(y, Some(&rows[y]), scale);
                assert_eq!(&mine[y * 320..y * 320 + 320], &theirs[..], "scale {scale} row {y}");
            }
        }
        // A clear (`linezoom(...,NULL,0)`) writes black inside the window and
        // leaves the fixed margins alone.
        let mut cleared = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
        zoom_row(&mut cleared, 0, None, 0);
        assert_eq!(&cleared[..32], &[0xAA; 32]);
        assert!(cleared[32..288].iter().all(|&v| v == 0));
        assert_eq!(&cleared[288..320], &[0xAA; 32]);
    }

    #[test]
    fn the_scale_dispatch_is_the_tables_and_the_reachable_range_stays_inside() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let render = |scale: i32| {
            let mut pixels = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
            zoom_row(&mut pixels, 0, Some(&rows[0]), scale);
            pixels[..PICTURE_WIDTH].to_vec()
        };
        // `DOL.C`'s table: 246..318 is `@zoom244`, 2..138 is `@zoom140`,
        // 140..244 is that label and 0 is `@zoom0`.
        assert_eq!(render(318), render(244));
        assert_eq!(render(320), render(244));
        assert_eq!(render(-3), render(244));
        assert_eq!(render(2), render(140));
        assert_eq!(render(141), render(140));
        let mut zero = vec![0xAAu8; PICTURE_WIDTH];
        zero[32..288].fill(0);
        assert_eq!(render(0), zero);
        // The part's own reachable scales, over every `doit` frame.
        let (fy1, fy2) = spring_tables();
        let (t1, t2) = interpolate_800(&fy1, &fy2);
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        for frame in 1..=FREEZE_AFTER as usize {
            let y1 = t1[frame] / 16;
            let y2 = t2[frame] / 16;
            let xsc = (400 - (y2 - y1)) / 8;
            let first = y1.max(0);
            let last = y2.min(PICTURE_HEIGHT as i32);
            for y in first..last {
                let b = (y - y1) * 400 / (y2 - y1);
                let a = 184
                    + (i32::from(SIN1024[b as usize * 32 / 25]) * xsc + 32) / 64;
                let a = a & !1;
                lo = lo.min(a);
                hi = hi.max(a);
            }
        }
        assert_eq!((lo, hi), (164, 224));
        assert!((140..=244).contains(&lo) && (140..=244).contains(&hi));
    }

    #[test]
    fn scale_184_is_the_identity_over_the_185_source_bytes() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let row = &rows[7];
        let mut surface = vec![0xAAu8; PICTURE_WIDTH * PICTURE_HEIGHT];
        zoom_row(&mut surface, 0, Some(row), 184);
        let out = &surface[..PICTURE_WIDTH];
        assert_eq!(out[68], row[0]);
        assert_eq!(out[251], row[183]);
        assert_eq!(out[252], BORDER_INDEX, "source index 184 is the 65 border");
        assert_eq!(out[67], 0);
        assert_eq!(out[253], 0);
    }

    // ----------------------------------------------------- the picture ----

    #[test]
    fn readp_is_the_sources_row_rle_and_palette() {
        let bytes = read_asset(NAME, PICTURE_PATH).unwrap();
        assert_eq!(u16_at(&bytes, 0), 0xFCFD, "FD FC, the RLE variant");
        assert_eq!(u16_at(&bytes, 2), 320);
        assert_eq!(u16_at(&bytes, 4), 400);
        assert_eq!(u16_at(&bytes, 6), 256);
        assert_eq!(u16_at(&bytes, 8), 49);
        let image = asset();
        assert_eq!(
            reference_palette(&bytes),
            image.palette.to_vga().iter().map(|b| b >> 2).collect::<Vec<u8>>(),
            "the palette at +16 is the asset's, entry for entry"
        );
        for row in 0..400 {
            let decoded = reference_readp_row(&bytes, row);
            assert_eq!(decoded.len(), 320, "row {row} decodes its 320 pixels");
            assert_eq!(
                &decoded[..],
                image.surface.row(row as usize),
                "row {row} of READP.C"
            );
        }
    }

    #[test]
    fn a_0x80_run_opcode_wraps_to_256_copies() {
        // `and ah,7fh` makes the count 0; `dec ah` wraps to 255 and the
        // `jnz` loop runs the body 256 times. The tree's writers never emit
        // it and `sr-assets` refuses it; this is the source's own reading.
        let mut out = Vec::new();
        rle_decode(&[0x80, 0x42, 0x05], &mut out);
        assert_eq!(out.len(), 257);
        assert!(out[..256].iter().all(|&v| v == 0x42));
        assert_eq!(out[256], 0x05);
    }

    #[test]
    fn the_rows_are_the_184_byte_crop_remapped_with_a_65_border() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        assert_eq!(rows.len(), PICTURE_HEIGHT);
        let bytes = read_asset(NAME, PICTURE_PATH).unwrap();
        for y in [0usize, 1, 100, 399] {
            let raw = reference_readp_row(&bytes, y as i32);
            for b in 0..CROP_WIDTH {
                let expected = if raw[CROP_LEFT + b] == 0 {
                    BLACK_INDEX
                } else {
                    raw[CROP_LEFT + b]
                };
                assert_eq!(rows[y][b], expected, "row {y} byte {b}");
            }
            assert_eq!(rows[y][CROP_WIDTH], BORDER_INDEX);
        }
        assert!(!rows.iter().any(|row| row[..CROP_WIDTH].contains(&0)));
    }

    #[test]
    fn the_picture_uses_only_colours_the_512_byte_upload_writes() {
        let image = asset();
        let rows = Jplogo::read_rows(&image);
        let mut highest = 0u8;
        for row in &rows {
            for &value in row {
                highest = highest.max(value);
                assert!(usize::from(value) < PALETTE_ENTRIES_WHOLE + 1);
            }
        }
        assert_eq!(highest, BORDER_INDEX);
    }

    // ----------------------------------------------------- the palette ----

    #[test]
    fn init_clears_the_dac_before_the_512_byte_upload() {
        // `inittwk`'s `mov cx,768 / out dx,al` (`ASM.ASM:31-38`) zeroes the
        // DAC before the upload, so an inherited palette cannot survive past
        // entry 169 — and entry 170's blue is that zero, not the old DAC's.
        let mut rig = real_rig();
        rig.palette = patterned_palette();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        let file = asset().palette;
        assert_eq!(
            rig.palette.entry(170),
            [file.entry(170)[0], file.entry(170)[1], 0],
            "entry 170's blue is the cleared DAC's"
        );
        for entry in 171..256u16 {
            assert_eq!(
                rig.palette.entry(entry as u8),
                [0, 0, 0],
                "entry {entry} was not written by the 512 bytes and stayed cleared"
            );
        }
        assert_eq!(rig.palette.entry(64), [0, 0, 0], "the blacked entry");
        assert_eq!(rig.palette.entry(1), file.entry(1), "the file's first entry");
    }

    #[test]
    fn the_palette_upload_is_the_sources_512_bytes() {
        let image = asset();
        let file = image.palette;
        let live = patterned_palette();
        let out = palette_upload(&file, live);
        assert_eq!(PALETTE_UPLOAD_BYTES, 512);
        for entry in 0..PALETTE_ENTRIES_WHOLE {
            let expected = if entry == usize::from(BLACK_INDEX) {
                [0, 0, 0]
            } else {
                file.entry(entry as u8)
            };
            assert_eq!(out.entry(entry as u8), expected, "entry {entry}");
        }
        // The 512th byte is entry 170's green; its blue is the DAC's old one.
        let partial = PALETTE_PARTIAL as u8;
        assert_eq!(
            out.entry(partial),
            [file.entry(partial)[0], file.entry(partial)[1], live.entry(partial)[2]]
        );
        for entry in PALETTE_PARTIAL + 1..256 {
            assert_eq!(out.entry(entry as u8), live.entry(entry as u8), "entry {entry}");
        }
        assert_eq!(file.entry(0), [0, 0, 0], "the picture's own entry 0 is black too");
    }

    // -------------------------------------------------------- the part ----

    /// A rig with the real player, `MUSIC1` seeked to order 96. The part sets
    /// no copper, so a step is the engine's poll and the part's `frame`.
    fn real_rig() -> Rig<PlayerMusic> {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(96);
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(PlayerMusic::new(player, 0))
    }

    #[test]
    fn the_view_is_320_by_400_at_80_plane_bytes_a_row() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        assert_eq!((rig.view.width, rig.view.height), (SCREEN_WIDTH, SCREEN_HEIGHT));
        assert!(!rig.view.double_scan, "CRTC 0x09 = 0");
        assert_eq!(rig.view.y_line_offset, Some(LINE_LEN));
        assert_eq!(rig.view.start_addr, 400 * 320, "scrolly(400)");
        assert_eq!(rig.view.pan, 0);
        assert_eq!(rig.view.split, None);
        assert_eq!(rig.view.border, Some(255), "AC 0x11 = 0xFF");
    }

    #[test]
    fn the_entry_ticks_walk_scroll_y_from_399_to_zero() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // Tick 0 shows the start-400 screen `init` left.
        rig.step(&mut part);
        assert_eq!(rig.view.start_addr, 400 * 320);
        let scrolls = entry_scrolls();
        for (step, &row) in scrolls.iter().enumerate() {
            rig.step(&mut part);
            assert_eq!(
                rig.view.start_addr,
                u32::from(row) * 320,
                "tick {} is entry step {step}",
                step + 1
            );
        }
        // The extra `dis_waitb()` before `doit()` holds start 0.
        rig.step(&mut part);
        assert_eq!(rig.view.start_addr, 0);
    }

    #[test]
    fn the_entry_pre_roll_reproduces_the_traces_positions() {
        assert_eq!(MUSIC_PRE_ROLL, 483);
        // The pre-roll's derivation: from a fresh seek, the published row-58
        // window begins at the 479th poll. The trace's row 58 begins at boot
        // 17060, five frames before tick 0's 17065, and tick 0's poll is the
        // (pre-roll + 1)th — so 483 + 1 - 5 = 479.
        {
            use crate::audio::{lock, test_player};
            use crate::shell::ModuleId;
            let player = test_player(ModuleId::Music1).unwrap();
            lock(&player).play_song(96);
            let mut music = PlayerMusic::new(player, 0);
            let mut polls = 0u32;
            loop {
                music.poll();
                polls += 1;
                let position = music.position();
                if position.order == 96 && position.row == 58 {
                    break;
                }
            }
            assert_eq!(polls, 479);
            assert_eq!(MUSIC_PRE_ROLL + 1 - 5, polls);
        }
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // The player is fresh from `_zplaysong(96)`; 483 polls leave it at
        // the row-58 window, whose start is five ticks before tick 0.
        let wanted = [(0u32, 96u8, 58u8), (3, 96, 59), (44, 98, 0), (76, 98, 4),
                      (84, 98, 5), (235, 98, 23), (435, 98, 48), (635, 98, 49)];
        for (tick, order, row) in wanted {
            while rig.tick < tick {
                rig.step(&mut part);
            }
            if rig.tick == tick {
                rig.step(&mut part);
            }
            assert_eq!(
                rig.player().position().order,
                order,
                "tick {tick}: the trace at {} is {order}:{row}",
                17065 + tick
            );
            assert_eq!(rig.player().position().row, row, "tick {tick}");
            assert_eq!(
                (rig.player().position().order, rig.player().position().row),
                trace_position("second3d", 17065 + tick),
                "tick {tick} against the trace"
            );
            if tick == 84 {
                // By the extra wait before `doit()` the gate reads
                // `musplus >= 4` (order 98 row 5).
                assert!(rig.player().musplus() >= 4);
            }
        }
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_poll() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        // `_zplaysong` publishes nothing until the ring drains: no pre-poll.
        assert_eq!(rig.player().position().order, 0);
        assert_eq!(rig.player().position().row, 0);
    }

    #[test]
    fn the_gate_waits_for_musplus_at_least_four() {
        let music = ScriptedMusic::musplus(vec![-6]).never_syncs("the gate does not sync");
        let mut rig = Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(music);
        let mut part = Jplogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        let mut tick = |part: &mut Jplogo, plus: i32, frame: u32| {
            rig.player().script = vec![Reading { musplus: plus, ..Reading::default() }];
            part.frame(&mut rig.ctx(frame))
        };
        // The entry, then the extra wait.
        for frame in 0..=EXTRA_WAIT_TICK {
            assert_eq!(tick(&mut part, -6, frame), PartStatus::Continuing);
        }
        // The first `doit` tick with the gate closed must not advance the
        // counter.
        assert_eq!(tick(&mut part, -6, DOIT_FIRST_TICK), PartStatus::Continuing);
        assert_eq!(part.frame, 0, "the gate has not opened");
        // `musplus == 4` is enough (`< 4` is the wait).
        assert_eq!(tick(&mut part, 4, DOIT_FIRST_TICK), PartStatus::Continuing);
        assert_eq!(part.frame, 1);
    }

    #[test]
    fn the_doit_guard_is_a_palimpsest() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.y1 = 0;
        part.y2 = 400;
        part.lasty[100] = 100;
        part.lasts[100] = 184;
        // A pixel the zoom would overwrite if the row were redrawn.
        rig.surface.pixels_mut()[100 * 320 + 100] = 0x77;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_eq!(
            rig.surface.pixels()[100 * 320 + 100],
            0x77,
            "row 100's (b,a) repeated, so it was not redrawn"
        );
        assert_eq!(part.lasty[101], 101);
        assert_eq!(part.lasts[101], 184);
        // Forgetting the row's state makes the next pass redraw it.
        part.lasty[100] = -1;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_ne!(rig.surface.pixels()[100 * 320 + 100], 0x77);
        // A row outside [y1,y2) is cleared in the zoom window and keeps the
        // fixed margins.
        part.y1 = 0;
        part.y2 = 100;
        rig.surface.pixels_mut()[200 * 320] = 0x33;
        rig.surface.pixels_mut()[200 * 320 + 100] = 0x44;
        part.draw_doit_frame(rig.surface.pixels_mut());
        assert_eq!(rig.surface.pixels()[200 * 320], 0x33, "margin untouched");
        assert_eq!(rig.surface.pixels()[200 * 320 + 100], 0);
    }

    #[test]
    fn the_counter_freezes_after_511_and_reads_the_table_before_it() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.gate_open = true;
        // Frame 510 takes the table's 511th entry, which is the settled
        // (0, 400).
        rig.tick = DOIT_FIRST_TICK;
        part.frame = 510;
        rig.step(&mut part);
        assert_eq!(part.frame, 511);
        assert_eq!((part.y1, part.y2), (0, 400));
        // The 512th leaves `y1`/`y2` alone: `if(frame>511) c=400;` never
        // assigns them, and `c` is not read anywhere again.
        part.y1 = 7;
        part.y2 = 123;
        rig.step(&mut part);
        assert_eq!(part.frame, 512);
        assert_eq!((part.y1, part.y2), (7, 123), "the freeze keeps the values");
    }

    #[test]
    fn scroll_lock_paints_the_border_the_source_paints() {
        // `JP.C:74-76`: `setborder(0)` around the wait and `setborder(127)`
        // after it. Nothing in a run sets Scroll Lock; the branch exists so
        // the source's structure is present and testable.
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.gate_open = true;
        part.scroll_lock = true;
        rig.tick = DOIT_FIRST_TICK;
        rig.step(&mut part);
        assert_eq!(rig.view.border, Some(127));
    }

    #[test]
    fn the_frame_cap_and_exit_end_the_part() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        part.gate_open = true;
        rig.tick = DOIT_FIRST_TICK;
        part.frame = FRAME_CAP;
        let status = rig.step(&mut part);
        assert_eq!(status, PartStatus::Finished);
        assert!(part.done);

        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        rig.tick = DOIT_FIRST_TICK;
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert!(part.done);
    }

    #[test]
    fn the_rendered_keyframes_are_the_references() {
        let mut rig = real_rig();
        let mut part = Jplogo::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // The view is at start 0 from the entry's last step on.
        for _ in 0..=235 {
            rig.step(&mut part);
        }
        assert_eq!(rig.tick, 236);
        let screen = usize::from(SCREEN_WIDTH) * usize::from(SCREEN_HEIGHT);
        let mine = rgb_of(&rig.surface.pixels()[..screen], &rig.palette);
        let differing = differing_pixels(&mine, &reference("jplogo", 17300, "rgb"));
        assert_eq!(differing, 0, "17300 at tick 235");
    }
}
