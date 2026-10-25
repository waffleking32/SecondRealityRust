//! MINVBALL — the mini vector balls: 512 dots over a grey floor.
//!
//! `DOTS/MAIN.C` (291 lines) is the part; `DOTS/ASM.ASM`'s `drawdots`
//! (`:52-218`) projects and draws the dots, and `DOTS/SIN1024.INC` is the sine
//! table both halves use. The acts, in order:
//!
//! 1. `:79-124` — `dis_partstart`, the 512-entry `dottaul` shuffled by 500
//!    random swaps, the dots set to one position and scrambled by 500 more,
//!    and the physics constants (`:102-113`).
//! 2. `:125-150` — mode 13h; the 16×4 colour ramp, 100 greys and white at
//!    255; the DAC read back into `pal` and then zeroed.
//! 3. `:152-167` — rows 100-199 of video memory filled with the rising index
//!    ramp, copied to `bgpic`, and the depth tables built.
//! 4. `:171-183` — the fade-in: 65 steps of **two** vblanks each, each step
//!    uploading `pal - b` clamped at 0.
//! 5. `:185-284` — the main loop: `while(!dis_exit() && frame<2450)`, with
//!    `repeat = dis_waitb()`, `setpalette(pal2)` once `frame>2300`, the demo
//!    cue `-4 < dis_musplus() < 0`, the stage physics and `drawdots()`.
//!
//! # The RNG is Microsoft C's, not Turbo C's
//!
//! The brief names Turbo C's `rand()`, but the source is the evidence and it
//! says otherwise: `DOTS/MAKEFILE:10-16` compiles with `cl /qc /AL` (Microsoft
//! C 6.0's large model), assembles with `tasm`, links with Microsoft's
//! `link /E`, and the linked EXE carries the **MS Run-Time Library** string.
//! Microsoft C 6.0's generator is a 32-bit LCG with **modulus 2³²**:
//!
//! ```text
//! seed = seed * 214013 + 2531011        (mod 2^32, seed starts at 1)
//! rand() = (seed >> 16) & 0x7FFF
//! ```
//!
//! Its first values are 41, 18,467, 6,334, 26,500, 19,169, … — the sequence
//! [`MsRand`] asserts. The frames decide it (D24): with this generator the
//! reference's dense 12,400 scatter frame differs from the port by **25
//! pixels** (`tests/minvball.rs`), where a generator with a different high
//! word places every dot elsewhere. `docs/fidelity/minvball.md` §2 records
//! the compile evidence and the measurement.
//!
//! The source calls `rand()` with no `srand`, so the seed is the library's
//! initial 1. Every use is integer: `rand()%512` in the two setup shuffles,
//! `rand()-16384`, `8000-rand()/2` and `rand()-16384` in the scatter.
//!
//! # The tick map, and the reference's numbers
//!
//! A tick is one `dis_waitb()` in the original. The port's engine gives one
//! tick per `frame()` call, so the port's `frame` counter (the C's `frame`)
//! advances once a tick. Tick 0 is the first of the fade-in's 130 waits; the
//! main loop's first iteration is tick [`MAIN_FIRST_TICK`] (130) and its
//! frame counter is 1.
//!
//! ```text
//! tick 0            the setup is done; the first fade wait
//! ticks 1-129 odd   the 65 fade uploads, `pal - b` for b = 64 down to 0
//! tick 130          the main loop's first iteration (frame 1)
//! ```
//!
//! The mapping **boot = 10,368 + tick** is pinned to ±1 by the survey's
//! 12,900 flat-grey frame: it is the fade-out's `c=59` upload, which puts
//! frame counter 2402/2403 at boot 12,900 (the sibling session's `c=57` is
//! the same curve one frame on) — a different capture and a different
//! observable. The four filed frames are reproduced by frame counters
//! 403, 703, 1303 and 1903 at ticks 532, 832, 1432 and 2032
//! (`tests/minvball.rs`), but those counters are the fit's own evidence, not
//! an independent check. 10,368 is the part's first frame inside the measured
//! window `(10,300, 10,400]` (`reference-indices.md` §14.5), and
//! 10,368 + 130 = 10,498 is the first main-loop draw. **This is a mapping,
//! not an anchor**: `shell::boot_anchor("minvball")` stays `None`, a run of
//! this part numbers its own frames from zero, and its manifest says
//! `index_origin: "part"`, `anchor: null`.
//!
//! # `repeat`, measured: it is one
//!
//! The original does `repeat = dis_waitb()` and advances the physics `repeat`
//! times per iteration; the port's engine gives one tick per call. The
//! reference's own frames measure the difference as **zero over the measured
//! interval**: the keyframes 10,900 / 11,200 / 11,800 / 12,400 are reproduced
//! at frame counters 403 / 703 / 1303 / 1903, whose differences (300 / 600 /
//! 600) are exactly the boot-index differences, and the four keyframes
//! reproduce at those counters (0 differing pixels in all four; `tests/minvball.rs`). A
//! `repeat` above one between two keyframes would advance the frame counter
//! without a draw and move the ball rows; the matches would fail. The run's
//! end at 12,947 is the mapping's prediction from the source's 2,450-frame
//! cap, not a measured frame.
//!
//! # The drawing, unit for unit
//!
//! `drawdots` (`ASM.ASM:52-218`) walks the 512 dots in array order. For each,
//! `z' = Z*cos - X*sin` gives `bp = z'+9000`; the screen x is
//! `((x'>>8)+(x'>>11))/bp + 160`, the shadow row `(8<<16)/bp + 100`, and the
//! ball row `(y<<6)/bp + 100`. The shadow is a two-pixel word of colour **87**
//! at `[bx]`; the ball is a two-pixel word at `bx+1`, a four-pixel dword at
//! `bx+320` and a two-pixel word at `bx+641`, all from `depthtable1/2/3`
//! indexed by `(bp>>6)&~3`. Before drawing, the dot's old shadow and old
//! three ball rows are restored from `bgpic`.
//!
//! **The gravity integration lives in the ball block and is skipped when the
//! shadow is off screen.** `ja @@2` (`:96`, `:105`) jumps past the whole ball
//! section, so a dot whose x or shadow row is outside the screen does not get
//! `yadd += gravity` that frame. The port keeps that control flow: an off
//! screen dot's `y` holds still until its shadow returns (see the note).
//!
//! # The bugs, preserved
//!
//! - **`mov ds:[si+6],ax`** (`ASM.ASM:194-196`) overwrites the stored shadow
//!   offset when the shadow is off screen. The three 32-bit restores just
//!   before it (`:186`, `:189`, `:192`) each leave `eax` holding the dword
//!   read from `bgpic[old2+640..644]`, so `ax` is that dword's **low word** —
//!   the first two background pixels of the old ball's third row — not the
//!   shadow's row (0 or 0x4040-0xA3A3 over the defined floor bytes). The next
//!   frame restores from that word as a pixel offset,
//!   which is why the port keeps the 65,536-byte video segment and a matching
//!   `bgpic`: the largest byte any write touches is 64,642 (a dword at
//!   `old2+640` with `old2 = 63,999`), and only the first 64,000 are
//!   displayed. `bgpic`'s bytes past 64,000 are the source's undefined read
//!   and the port's zero fill (D24).
//! - **The shadow colour 87 sits inside the background's grey ramp** (64-163).
//!   It is written as it stands, over the floor.
//! - **The C's `mov ax,ax`** (`ASM.ASM:65`) is a no-op where the rotation's
//!   low word should have been saved (`mov bx,ax`). The z rotation is `sub
//!   ax,bx` / `sub dx,cx` (`:69-70`): `sub dx,cx` ignores the carry and `ax`
//!   is overwritten at `:74`, so the stale `bx` never reaches `bp`, which is
//!   `hi(Z*cos) - hi(X*sin) + 9000` (`bugs.md` §18.1). The port once
//!   subtracted a borrow from the previous dot's offset; that was its own
//!   defect and is gone.
//! - **`j` is never initialized** (`MAIN.C:87`): the first event reads
//!   `dottaul[j]` at whatever the stack held. The reference's overlaps pin it
//!   to **88**; the port starts there ([`INITIAL_J`]), and the ignored
//!   measurement test `the_initial_j_is_fitted_to_the_reference` replays all
//!   512 values against the 10,900 frame and asserts 88 is the minimum (0
//!   pixels; the next is 283).
//!
//! # The cue, and `dis_indemo`
//!
//! `:191-195` breaks the loop when `dis_musplus()` is in `(-4, 0)`, inside an
//! `if(dis_indemo())`. The port has no demo flag: the run is always "in
//! demo" and the cue applies to standalone runs too. The part's `standalone`
//! flag decides nothing here, which is the controller's ruling for the phase
//! and the note records it. The player publishes -3 from boot 10,368
//! (`reference/music/second3b/trace.csv`, order 75 row 61) — during the fade,
//! which does not check the cue — and the first -3 the main loop's own check
//! would see is boot **12,950**, three frames after the frame cap (12,947)
//! has already ended the loop. The cue never fires in the reference run; the
//! port's cap wins there too. The part declines D28's held-reading pattern
//! (see the note §5): it reads the engine's tick-own poll, and the eligible
//! reading is past the cap, so the one-tick difference changes no picture.

use sr_engine::part::{Part, PartCtx, PartStatus};
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "minvball";

// ------------------------------------------------------------- the screen --

/// `MAIN.C:125` `mov ax,13h`: 320 pixels a row, 200 rows, one byte a pixel.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// Mode 13h's row stride. The view names it explicitly so the surface's own
/// 640-pixel stride cannot shear the picture.
const LINE_LEN: u16 = 320;
/// The displayed picture: 320×200 one-byte pixels.
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
/// What the drawing can address: `bx` is a 16-bit word, so video memory is
/// the whole 64 KB and the last 1,536 bytes are off screen but writable
/// (`ASM.ASM:177`'s `bx+641` reaches 64,640).
const FRAME_MEMORY: usize = 65_536;

// -------------------------------------------------------------- the dots --

/// `MAIN.C:91` `dotnum=512`.
const DOTNUM: usize = 512;
/// `MAIN.C:84` `int rota=-1*64`. `rot`, by contrast, is initialized to **0**
/// (`MAIN.C:86` `int rot=0,rots=0`).
const ROTA_START: i32 = -64;
/// `MAIN.C:102-113`: `dropper=22000`, `grav=3`, `gravd=13`,
/// `gravitybottom=8105`.
const DROPPER_START: i16 = 22_000;
const GRAV_START: i16 = 3;
const GRAVD: i16 = 13;
const GRAVITY_BOTTOM: i16 = 8105;
/// `MAIN.C:270` `if(dropper>4000) dropper-=100`.
const DROPPER_FLOOR: i16 = 4000;
/// `MAIN.C:134`'s depth tables are 128 dwords each.
const DEPTH_ENTRIES: usize = 128;
/// The shadow's colour: `ASM.ASM:116` `mov ax,87+87*256`.
const SHADOW: u8 = 87;

/// The initial `j`. The source never assigns it (`MAIN.C:87` declares `j`
/// beside the initialized locals and `:200` reads it), so every value is
/// permitted by the source; the reference's overlap resolution selects 88.
/// See the module docs and `docs/fidelity/minvball.md` §5.
const INITIAL_J: usize = 88;

// -------------------------------------------------------------- the clock --

/// `MAIN.C:185` `frame<2450`: the main loop draws 2,450 frames (1 through
/// 2,450) and the next tick leaves.
const LAST_FRAME: i32 = 2450;
/// `MAIN.C:189` `if(frame>2300) setpalette(pal2)`.
const PALETTE_AFTER: i32 = 2300;
/// `MAIN.C:2360` is where the palette's blue ramp starts.
const BLUE_RAMP_FRAME: i32 = 2360;
/// `MAIN.C:2400` is where the fade to black starts.
const FADE_FRAME: i32 = 2400;
/// `MAIN.C:2440` is where the palette stops changing.
const FADE_END_FRAME: i32 = 2440;
/// `MAIN.C:171-183` `for(b=64;b>=0;b--)` with two `dis_waitb()` calls a step:
/// 65 uploads and 130 waiting ticks.
const FADE_STEPS: u32 = 65;
const FADE_WAITS: u32 = FADE_STEPS * 2;
/// The tick the main loop's first iteration runs on.
const MAIN_FIRST_TICK: u32 = FADE_WAITS;

/// The reference's mapping: tick 0 is boot frame 10,368, and the first
/// main-loop iteration (frame counter 1) is boot 10,498. **A mapping, not an
/// anchor** — only the tests compare by it.
#[cfg(test)]
const REFERENCE_FIRST_INDEX: u32 = 10_368;
#[cfg(test)]
const MAIN_FIRST_FRAME_INDEX: u32 = 10_498;

// ---------------------------------------------------------------- the cue --

/// `MAIN.C:191-195`: `if(a>-4 && a<0) break` with `a = dis_musplus()`.
const CUE_LOW: i32 = -4;
const CUE_HIGH: i32 = 0;

/// Polls `init` makes before the run's first tick, so that the player is where
/// the reference's was at the part's tick 0.
///
/// The shell seeks `MUSIC1` to order **75**, the order the trace holds when
/// the part's start window opens (`reference/music/second3b/trace.csv` frames
/// 10,300-10,368). The reference's published position at boot 10,368 — the
/// part's tick 0 — is **75:61** (`zplus` 1, `musplus` -3); a player seeked to
/// order 75 reaches that on its 257th poll, so **256** pre-polls leave the
/// engine's tick-0 poll as that 257th. The main loop then starts at tick 130
/// with the published position in order 77 (the trace's 77:29 region at boot
/// 10,498), and order 75's cue rows (61-63, `musplus` -3..-1) fall inside the
/// fade, where the source does not check the cue.
const MUSIC_PRE_ROLL: u32 = 256;

// -------------------------------------------------------------- the RNG ---

/// Microsoft C 6.0's `rand()`: a 32-bit LCG, modulus 2³², seed 1, returning
/// `(seed >> 16) & 0x7FFF`. See the module docs for the evidence that this,
/// not Turbo C's generator, is the source's.
pub(crate) struct MsRand {
    seed: u32,
}

impl MsRand {
    /// The multiplier and increment of Microsoft's generator.
    const MULTIPLIER: u32 = 214_013;
    const INCREMENT: u32 = 2_531_011;
    /// The modulus, `2^32`; the state is a `u32`, so the multiply wraps.
    #[cfg(test)]
    const MODULUS: u64 = 1 << 32;

    /// `holdrand = 1L` — the library's initial seed; the source never seeds.
    pub(crate) fn new() -> MsRand {
        MsRand { seed: 1 }
    }

    /// One `rand()`: 0 through 32,767.
    pub(crate) fn next(&mut self) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(Self::INCREMENT);
        ((self.seed >> 16) & 0x7FFF) as i32
    }
}

// ------------------------------------------------------------- the tables --

/// `SIN1024.INC`: sin(2π·deg/1024)·256, transcribed. `isin` indexes it
/// directly and `icos` at `deg+256`.
const SIN1024: [i16; 1024] = [
    0, 1, 3, 4, 6, 7, 9, 10, 12, 14, 15, 17, 18, 20, 21, 23, 25, 26, 28, 29, 31, 32, 34, 36, 37,
    39, 40, 42, 43, 45, 46, 48, 49, 51, 53, 54, 56, 57, 59, 60, 62, 63, 65, 66, 68, 69, 71, 72, 74,
    75, 77, 78, 80, 81, 83, 84, 86, 87, 89, 90, 92, 93, 95, 96, 97, 99, 100, 102, 103, 105, 106,
    108, 109, 110, 112, 113, 115, 116, 117, 119, 120, 122, 123, 124, 126, 127, 128, 130, 131, 132,
    134, 135, 136, 138, 139, 140, 142, 143, 144, 146, 147, 148, 149, 151, 152, 153, 155, 156, 157,
    158, 159, 161, 162, 163, 164, 166, 167, 168, 169, 170, 171, 173, 174, 175, 176, 177, 178, 179,
    181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199,
    200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 211, 212, 213, 214, 215, 216, 217,
    217, 218, 219, 220, 221, 221, 222, 223, 224, 225, 225, 226, 227, 227, 228, 229, 230, 230, 231,
    232, 232, 233, 234, 234, 235, 235, 236, 237, 237, 238, 238, 239, 239, 240, 241, 241, 242, 242,
    243, 243, 244, 244, 244, 245, 245, 246, 246, 247, 247, 247, 248, 248, 249, 249, 249, 250, 250,
    250, 251, 251, 251, 251, 252, 252, 252, 252, 253, 253, 253, 253, 254, 254, 254, 254, 254, 254,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 256, 255, 255, 255, 255,
    255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 254, 254, 254, 254, 254, 254, 253, 253, 253,
    253, 252, 252, 252, 252, 251, 251, 251, 251, 250, 250, 250, 249, 249, 249, 248, 248, 247, 247,
    247, 246, 246, 245, 245, 244, 244, 244, 243, 243, 242, 242, 241, 241, 240, 239, 239, 238, 238,
    237, 237, 236, 235, 235, 234, 234, 233, 232, 232, 231, 230, 230, 229, 228, 227, 227, 226, 225,
    225, 224, 223, 222, 221, 221, 220, 219, 218, 217, 217, 216, 215, 214, 213, 212, 211, 211, 210,
    209, 208, 207, 206, 205, 204, 203, 202, 201, 200, 199, 198, 197, 196, 195, 194, 193, 192, 191,
    190, 189, 188, 187, 186, 185, 184, 183, 182, 181, 179, 178, 177, 176, 175, 174, 173, 171, 170,
    169, 168, 167, 166, 164, 163, 162, 161, 159, 158, 157, 156, 155, 153, 152, 151, 149, 148, 147,
    146, 144, 143, 142, 140, 139, 138, 136, 135, 134, 132, 131, 130, 128, 127, 126, 124, 123, 122,
    120, 119, 117, 116, 115, 113, 112, 110, 109, 108, 106, 105, 103, 102, 100, 99, 97, 96, 95, 93,
    92, 90, 89, 87, 86, 84, 83, 81, 80, 78, 77, 75, 74, 72, 71, 69, 68, 66, 65, 63, 62, 60, 59, 57,
    56, 54, 53, 51, 49, 48, 46, 45, 43, 42, 40, 39, 37, 36, 34, 32, 31, 29, 28, 26, 25, 23, 21, 20,
    18, 17, 15, 14, 12, 10, 9, 7, 6, 4, 3, 1, 0, -1, -3, -4, -6, -7, -9, -10, -12, -14, -15, -17,
    -18, -20, -21, -23, -25, -26, -28, -29, -31, -32, -34, -36, -37, -39, -40, -42, -43, -45, -46,
    -48, -49, -51, -53, -54, -56, -57, -59, -60, -62, -63, -65, -66, -68, -69, -71, -72, -74, -75,
    -77, -78, -80, -81, -83, -84, -86, -87, -89, -90, -92, -93, -95, -96, -97, -99, -100, -102,
    -103, -105, -106, -108, -109, -110, -112, -113, -115, -116, -117, -119, -120, -122, -123, -124,
    -126, -127, -128, -130, -131, -132, -134, -135, -136, -138, -139, -140, -142, -143, -144, -146,
    -147, -148, -149, -151, -152, -153, -155, -156, -157, -158, -159, -161, -162, -163, -164, -166,
    -167, -168, -169, -170, -171, -173, -174, -175, -176, -177, -178, -179, -181, -182, -183, -184,
    -185, -186, -187, -188, -189, -190, -191, -192, -193, -194, -195, -196, -197, -198, -199, -200,
    -201, -202, -203, -204, -205, -206, -207, -208, -209, -210, -211, -211, -212, -213, -214, -215,
    -216, -217, -217, -218, -219, -220, -221, -221, -222, -223, -224, -225, -225, -226, -227, -227,
    -228, -229, -230, -230, -231, -232, -232, -233, -234, -234, -235, -235, -236, -237, -237, -238,
    -238, -239, -239, -240, -241, -241, -242, -242, -243, -243, -244, -244, -244, -245, -245, -246,
    -246, -247, -247, -247, -248, -248, -249, -249, -249, -250, -250, -250, -251, -251, -251, -251,
    -252, -252, -252, -252, -253, -253, -253, -253, -254, -254, -254, -254, -254, -254, -255, -255,
    -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -256, -255, -255, -255,
    -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -255, -254, -254, -254, -254, -254,
    -254, -253, -253, -253, -253, -252, -252, -252, -252, -251, -251, -251, -251, -250, -250, -250,
    -249, -249, -249, -248, -248, -247, -247, -247, -246, -246, -245, -245, -244, -244, -244, -243,
    -243, -242, -242, -241, -241, -240, -239, -239, -238, -238, -237, -237, -236, -235, -235, -234,
    -234, -233, -232, -232, -231, -230, -230, -229, -228, -227, -227, -226, -225, -225, -224, -223,
    -222, -221, -221, -220, -219, -218, -217, -217, -216, -215, -214, -213, -212, -211, -211, -210,
    -209, -208, -207, -206, -205, -204, -203, -202, -201, -200, -199, -198, -197, -196, -195, -194,
    -193, -192, -191, -190, -189, -188, -187, -186, -185, -184, -183, -182, -181, -179, -178, -177,
    -176, -175, -174, -173, -171, -170, -169, -168, -167, -166, -164, -163, -162, -161, -159, -158,
    -157, -156, -155, -153, -152, -151, -149, -148, -147, -146, -144, -143, -142, -140, -139, -138,
    -136, -135, -134, -132, -131, -130, -128, -127, -126, -124, -123, -122, -120, -119, -117, -116,
    -115, -113, -112, -110, -109, -108, -106, -105, -103, -102, -100, -99, -97, -96, -95, -93, -92,
    -90, -89, -87, -86, -84, -83, -81, -80, -78, -77, -75, -74, -72, -71, -69, -68, -66, -65, -63,
    -62, -60, -59, -57, -56, -54, -53, -51, -49, -48, -46, -45, -43, -42, -40, -39, -37, -36, -34,
    -32, -31, -29, -28, -26, -25, -23, -21, -20, -18, -17, -15, -14, -12, -10, -9, -7, -6, -4, -3,
    -1,
];

/// `MAIN.C:32-40`: `isin(deg) = sin1024[deg&1023]`.
fn isin(deg: i32) -> i32 {
    i32::from(SIN1024[(deg & 1023) as usize])
}

/// `MAIN.C:37-40`: `icos(deg) = sin1024[(deg+256)&1023]`.
fn icos(deg: i32) -> i32 {
    i32::from(SIN1024[((deg + 256) & 1023) as usize])
}

// --------------------------------------------------------------- the dot --

/// One `struct dot` entry (`MAIN.C:42-52`, `ASM.ASM:36`): sixteen-bit ints in
/// memory, `old1` the stored shadow offset and `old2` the stored ball offset.
#[derive(Clone, Copy, Default)]
struct Dot {
    x: i16,
    y: i16,
    z: i16,
    yadd: i16,
    old1: u16,
    old2: u16,
}

// -------------------------------------------------------------- the part --

/// Where in `MAIN.C`'s flow the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `:171-183`: the 130 waiting ticks and their 65 uploads.
    Fade,
    /// `:185-284`: the main loop.
    Main,
    /// The loop has ended; the part keeps its last frame.
    Done,
}

/// MINVBALL: the 512 dots, their background and the two palettes.
pub struct Minvball {
    dots: Vec<Dot>,
    /// `MAIN.C:77` `dottaul[1024]` — the port keeps the 512 used entries.
    order: Vec<u16>,
    /// `MAIN.C:200`'s `j`, the read cursor into `order`.
    next: usize,
    /// `MAIN.C:83` `frame`.
    frame: i32,
    /// `MAIN.C:89` `f`, the stage animation phase.
    f: i32,
    dropper: i16,
    grav: i16,
    gravd: i16,
    gravitybottom: i16,
    gravity: i16,
    /// `MAIN.C:84-86`.
    rota: i32,
    rot: i32,
    rots: i32,
    rotcos: i32,
    rotsin: i32,
    /// `MAIN.C:27-28` `char pal[768], pal2[768]`: six-bit channels.
    pal: [u8; 768],
    pal2: [u8; 768],
    /// `MAIN.C:16-19` `depthtable1/2/3`, 128 dwords each.
    depth1: [u32; DEPTH_ENTRIES],
    depth2: [u32; DEPTH_ENTRIES],
    depth3: [u32; DEPTH_ENTRIES],
    /// `MAIN.C:13` `bgpic`, `halloc(64000)`; the port keeps the 64 KB segment
    /// so the buggy row-as-offset restore reads defined bytes.
    bgpic: Vec<u8>,
    /// `MAIN.C:25` `vram`, the 64 KB VGA segment.
    video: Vec<u8>,
    rng: MsRand,
    /// The fade's tick counter, 0 through 129.
    fade_tick: u32,
    phase: Phase,
    /// The part's tick counter (the dump index), advanced at the end of a
    /// call: the call that draws dump index `n` leaves this at `n + 1`.
    tick: u32,
}

impl Minvball {
    /// A part with no state yet; `init` builds everything.
    pub fn new() -> Minvball {
        Minvball {
            dots: Vec::new(),
            order: Vec::new(),
            next: INITIAL_J,
            frame: 0,
            f: 0,
            dropper: DROPPER_START,
            grav: GRAV_START,
            gravd: GRAVD,
            gravitybottom: GRAVITY_BOTTOM,
            gravity: GRAV_START,
            rota: ROTA_START,
            rot: 0,
            rots: 0,
            rotcos: 0,
            rotsin: 0,
            pal: [0; 768],
            pal2: [0; 768],
            depth1: [0; DEPTH_ENTRIES],
            depth2: [0; DEPTH_ENTRIES],
            depth3: [0; DEPTH_ENTRIES],
            bgpic: vec![0; FRAME_MEMORY],
            video: vec![0; FRAME_MEMORY],
            rng: MsRand::new(),
            fade_tick: 0,
            phase: Phase::Fade,
            tick: 0,
        }
    }

    /// `MAIN.C:92-123`: the `dottaul` shuffle, the initial positions and the
    /// scramble. The scramble moves nothing — every dot starts at the same
    /// `(0, 2560-dropper, 0)` — but it consumes its 1,000 `rand()` calls, and
    /// the scatter's stream position depends on them; the source's loop is
    /// kept, calls and all.
    fn setup_dots(rng: &mut MsRand) -> (Vec<u16>, Vec<Dot>) {
        let mut order: Vec<u16> = (0..DOTNUM as u16).collect();
        for _ in 0..500 {
            let b = (rng.next() % DOTNUM as i32) as usize;
            let c = (rng.next() % DOTNUM as i32) as usize;
            order.swap(b, c);
        }
        let mut dots = vec![Dot::default(); DOTNUM];
        for dot in &mut dots {
            dot.x = 0;
            dot.y = 2560 - DROPPER_START;
            dot.z = 0;
            dot.yadd = 0;
        }
        for _ in 0..500 {
            let b = (rng.next() % DOTNUM as i32) as usize;
            let c = (rng.next() % DOTNUM as i32) as usize;
            let (x, y, z) = (dots[b].x, dots[b].y, dots[b].z);
            dots[b].x = dots[c].x;
            dots[b].y = dots[c].y;
            dots[b].z = dots[c].z;
            dots[c].x = x;
            dots[c].y = y;
            dots[c].z = z;
        }
        (order, dots)
    }

    /// `MAIN.C:127-147`: the palette the DAC is given and then read back.
    fn build_palette() -> [u8; 768] {
        // `MAIN.C:72-76` `cols[]`.
        const COLS: [[u8; 3]; 4] = [[0, 0, 0], [4, 25, 30], [8, 40, 45], [16, 55, 60]];
        let mut pal = [0u8; 768];
        // `:128-134` `for(a=0;a<16;a++) for(b=0;b<4;b++)`, c = 100+a*9.
        for a in 0..16usize {
            let c = 100 + a as u32 * 9;
            for b in 0..4usize {
                pal[(a * 4 + b) * 3] = COLS[b][0];
                pal[(a * 4 + b) * 3 + 1] = (u32::from(COLS[b][1]) * c / 256) as u8;
                pal[(a * 4 + b) * 3 + 2] = (u32::from(COLS[b][2]) * c / 256) as u8;
            }
        }
        // `:135-138` entry 255.
        pal[255 * 3] = 31;
        pal[255 * 3 + 1] = 0;
        pal[255 * 3 + 2] = 15;
        // `:139-147` the 100 greys at entries 64-163.
        for a in 0..100u32 {
            let c = 64 - 256 / (a + 4);
            let c = c * c / 64;
            let v = (c / 4) as u8;
            pal[(64 + a as usize) * 3] = v;
            pal[(64 + a as usize) * 3 + 1] = v;
            pal[(64 + a as usize) * 3 + 2] = v;
        }
        pal
    }

    /// `MAIN.C:156-167`: one depth table's value for entry `a`.
    fn depth_value(a: usize, base: u32) -> u32 {
        // `c=a-(43+20)/2; c=c*3/4; c+=8;` clamp to 0..15; `c=15-c`.
        let c = a as i32 - (43 + 20) / 2;
        // C's integer division truncates toward zero.
        let c = c * 3 / 4;
        let c = (c + 8).clamp(0, 15);
        let c = 15 - c;
        base.wrapping_add(0x0404_0404u32.wrapping_mul(c as u32))
    }

    /// The three tables `MAIN.C:163-166` builds (`depthtable4` is commented
    /// out and unused).
    fn build_depth_tables() -> ([u32; DEPTH_ENTRIES], [u32; DEPTH_ENTRIES], [u32; DEPTH_ENTRIES]) {
        let mut d1 = [0u32; DEPTH_ENTRIES];
        let mut d2 = [0u32; DEPTH_ENTRIES];
        let mut d3 = [0u32; DEPTH_ENTRIES];
        for a in 0..DEPTH_ENTRIES {
            d1[a] = Self::depth_value(a, 0x0202);
            d2[a] = Self::depth_value(a, 0x0203_0302);
            d3[a] = Self::depth_value(a, 0x0202);
        }
        (d1, d2, d3)
    }

    /// `MAIN.C:125-126`: mode 13h and the display geometry the port's view
    /// needs to see the packed 320-wide picture.
    fn set_display(ctx: &mut PartCtx) {
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = SCREEN_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(LINE_LEN);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
    }

    /// The DAC from `pal2`, the array the fades write.
    fn palette_of(pal2: &[u8; 768]) -> Palette {
        let mut dac = Palette::new();
        for entry in 0..256usize {
            dac.set_entry(
                entry as u8,
                [pal2[entry * 3], pal2[entry * 3 + 1], pal2[entry * 3 + 2]],
            );
        }
        dac
    }

    /// One iteration of `MAIN.C:196-282`'s `while(repeat--)`. The port's
    /// engine gives one repeat.
    fn physics(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        if frame == 500 {
            self.f = 0;
        }
        let i = self.order[self.next] as usize;
        self.next = (self.next + 1) % DOTNUM;
        if frame < 500 {
            // `:202-208`.
            self.dots[i].x = (isin(self.f * 11) * 40) as i16;
            self.dots[i].y = (icos(self.f * 13) * 10 - i32::from(self.dropper)) as i16;
            self.dots[i].z = (isin(self.f * 17) * 40) as i16;
            self.dots[i].yadd = 0;
        } else if frame < 900 {
            // `:209-215`.
            self.dots[i].x = (icos(self.f * 15) * 55) as i16;
            self.dots[i].y = self.dropper;
            self.dots[i].z = (isin(self.f * 15) * 55) as i16;
            self.dots[i].yadd = -260;
        } else if frame < 1700 {
            // `:216-223`.
            let a = isin(frame & 1023) / 8;
            self.dots[i].x = (icos(self.f * 66) * a) as i16;
            self.dots[i].y = 8000;
            self.dots[i].z = (isin(self.f * 66) * a) as i16;
            self.dots[i].yadd = -300;
        } else if frame < BLUE_RAMP_FRAME {
            // `:224-241`.
            self.dots[i].x = (self.rng.next() - 16384) as i16;
            self.dots[i].y = (8000 - self.rng.next() / 2) as i16;
            self.dots[i].z = (self.rng.next() - 16384) as i16;
            self.dots[i].yadd = 0;
            if frame > 1900 && frame & 31 == 0 && self.grav > 0 {
                self.grav -= 1;
            }
        } else if frame < FADE_FRAME {
            // `:242-257`: the blue ramp.
            let a = frame - BLUE_RAMP_FRAME;
            for b in (0..768).step_by(3) {
                self.pal2[b] = (i32::from(self.pal[b]) + a * 3).min(63) as u8;
                self.pal2[b + 1] = (i32::from(self.pal[b + 1]) + a * 3).min(63) as u8;
                self.pal2[b + 2] = (i32::from(self.pal[b + 2]) + a * 4).min(63) as u8;
            }
        } else if frame < FADE_END_FRAME {
            // `:258-269`: every entry down together.
            let a = frame - FADE_FRAME;
            let c = (63 - a * 2).max(0) as u8;
            self.pal2.fill(c);
        }
        if self.dropper > DROPPER_FLOOR {
            self.dropper -= 100;
        }
        // `:271-281`.
        self.rotcos = icos(self.rot) * 64;
        self.rotsin = isin(self.rot) * 64;
        self.rots += 2;
        if frame > 1900 {
            self.rot += self.rota / 64;
            self.rota -= 1;
        } else {
            self.rot = isin(self.rots);
        }
        self.f += 1;
        self.gravity = self.grav;
        self.gravd = GRAVD;
    }

    /// One fade tick: the first wait, then (on odd ticks) one upload.
    fn fade(&mut self, ctx: &mut PartCtx) {
        let t = self.fade_tick;
        if t % 2 == 1 {
            let step = (t - 1) / 2;
            let b = (FADE_STEPS - 1 - step) as i32;
            for c in 0..768 {
                self.pal2[c] = (i32::from(self.pal[c]) - b).max(0) as u8;
            }
            ctx.set_palette(Self::palette_of(&self.pal2));
        }
        self.fade_tick += 1;
        if self.fade_tick == MAIN_FIRST_TICK {
            self.phase = Phase::Main;
        }
    }

    /// `MAIN.C:185-284`'s loop body for one tick.
    fn main_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `while(!dis_exit() && frame<2450)`: the source checks the exit
        // first; the port's engine has already set the flag.
        if ctx.exit_requested {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        if self.frame >= LAST_FRAME {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        // `:189`.
        if self.frame > PALETTE_AFTER {
            ctx.set_palette(Self::palette_of(&self.pal2));
        }
        // `:191-195`. There is no `dis_indemo` in the port: the run is always
        // in demo, and a standalone run keeps the cue (module docs).
        let reading = ctx.music().map_or(-32, |music| music.musplus());
        if reading > CUE_LOW && reading < CUE_HIGH {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        self.physics();
        self.draw(ctx);
        PartStatus::Continuing
    }

    /// `ASM.ASM:52-218`'s `drawdots` into the 64 KB video buffer.
    fn draw_video(&mut self) {
        drawdots(
            &mut self.dots,
            &self.bgpic,
            &mut self.video,
            self.rotcos,
            self.rotsin,
            self.gravity,
            self.gravd,
            self.gravitybottom,
            &self.depth1,
            &self.depth2,
            &self.depth3,
        );
    }

    /// One drawing pass, then the visible 64,000 bytes to the surface.
    fn draw(&mut self, ctx: &mut PartCtx) {
        self.draw_video();
        let memory = ctx.surface.pixels_mut();
        memory[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
    }

    /// The deterministic state `init` builds (`MAIN.C:92-169`), from the live
    /// DAC's entries 164-254 and a fresh RNG.
    ///
    /// Factored out of [`init`](Minvball::init) so the `j`-fit measurement test
    /// can build the state without a `PartCtx`. It resets the RNG first, so a
    /// re-init of the same instance reproduces the source's seed-1 setup.
    fn setup_state(&mut self, live: &Palette) {
        self.rng = MsRand::new();

        // `:92-123`. The scramble consumes the same calls as the source even
        // though it moves nothing (every dot starts identical).
        let (order, dots) = Self::setup_dots(&mut self.rng);
        self.order = order;
        self.dots = dots;
        self.next = INITIAL_J;

        // `:127-150`: build the palette, read it back. The DAC before the
        // part is what entries 164-254 keep, exactly as the source's
        // read-back of the live hardware does.
        let mut pal = [0u8; 768];
        for entry in 0..256usize {
            let rgb = live.entry(entry as u8);
            pal[entry * 3..entry * 3 + 3].copy_from_slice(&rgb);
        }
        let built = Self::build_palette();
        pal[..64 * 3].copy_from_slice(&built[..64 * 3]);
        pal[64 * 3..164 * 3].copy_from_slice(&built[64 * 3..164 * 3]);
        pal[255 * 3..255 * 3 + 3].copy_from_slice(&built[255 * 3..255 * 3 + 3]);
        self.pal = pal;
        self.pal2 = pal;

        // `:152-155`: the mode set's clear and rows 100-199's ramp.
        self.video.fill(0);
        for a in 0..100usize {
            let row = 100 + a;
            self.video[row * 320..(row + 1) * 320].fill((a + 64) as u8);
        }
        // `:168-169` `bgpic=halloc(64000); memcpy(bgpic,vram,64000)`. The
        // 1,536 bytes past 64,000 stay zero: the source's own reads there are
        // out of the allocation, and D24 takes the defined fill.
        self.bgpic.fill(0);
        self.bgpic[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);

        // `:156-167`.
        let (d1, d2, d3) = Self::build_depth_tables();
        self.depth1 = d1;
        self.depth2 = d2;
        self.depth3 = d3;

        // `MAIN.C:102-113`'s runtime constants.
        self.dropper = DROPPER_START;
        self.grav = GRAV_START;
        self.gravd = GRAVD;
        self.gravitybottom = GRAVITY_BOTTOM;
        self.gravity = GRAV_START;
        self.rota = ROTA_START;
        self.rot = 0;
        self.rots = 0;
        self.rotcos = 0;
        self.rotsin = 0;
        self.f = 0;
        self.frame = 0;
        self.fade_tick = 0;
        self.phase = Phase::Fade;
        self.tick = 0;
    }
}

impl Part for Minvball {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:79-183`: the deterministic setup, the palette, the floor, the
    /// tables and the fade's state. The source runs all of this before its
    /// first `dis_waitb`; the port's `init` is the same place.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), sr_engine::part::PartError> {
        // The run's player starts with the entry part and is seeked to order
        // 75; the reference's player was already mid-order-75 at this part's
        // tick 0. A part reached mid-chain starts on a player that has already
        // run through this stretch, so the catch-up belongs to the entry run
        // (`music_fresh`) alone. A player with no music (a unit-test rig, the
        // test card) has no position to align; the part does not need one to
        // run.
        if ctx.music_fresh() {
            if let Some(music) = ctx.music() {
                for _ in 0..MUSIC_PRE_ROLL {
                    music.poll();
                }
            }
        }

        let live = *ctx.palette;
        self.setup_state(&live);
        // `:151`: the DAC zeroed after the read-back.
        ctx.set_palette(Palette::new());

        // The mode set's picture is up before the fade's first wait.
        Self::set_display(ctx);
        let memory = ctx.surface.pixels_mut();
        memory[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
        Ok(())
    }

    /// One tick of `MAIN.C:171-284`.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Fade => {
                // The fade is 130 ticks, not a spin, but the port honours the
                // exit notice here as every part does.
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                self.fade(ctx);
                PartStatus::Continuing
            }
            Phase::Main => self.main_tick(ctx),
        }
    }
}

// ------------------------------------------------------------ the drawing --

/// `ASM.ASM:61-92`: the shared x/z projection. Returns `(x, shadow row, bp)`;
/// `bp` also gives the depth entry (`bp >> 8`) and is reused for the ball row
/// after the gravity integration.
fn project_xz(dot: &Dot, rotcos: i32, rotsin: i32) -> (i16, i16, i16) {
    // The z rotation (`:61-72`): `X*sin`'s high word goes to `cx` (`mov
    // cx,dx`), then `Z*cos`'s product is reduced by `sub ax,bx` / `sub dx,cx`.
    // `sub dx,cx` ignores the carry and `ax` is overwritten at `:74`, so the
    // low word, the stale `bx` and the `mov ax,ax` (`:65`) never reach `bp`:
    // it is the difference of the high words, plus 9000.
    let xsin = i32::from(dot.x) * rotsin;
    let zcos = i32::from(dot.z) * rotcos;
    let high = ((zcos >> 16) as u16).wrapping_sub((xsin >> 16) as u16);
    let bp = (high as i16).wrapping_add(9000);

    // `:74-93`: x' = X*cos + Z*sin; the screen x is (x'>>8 + x'>>11)/bp+160.
    let x32 = i32::from(dot.x)
        .wrapping_mul(rotcos)
        .wrapping_add(i32::from(dot.z).wrapping_mul(rotsin));
    let v = (x32 >> 8).wrapping_add(x32 >> 11);
    let x = (v / i32::from(bp)) as i16;
    let x = x.wrapping_add(160);

    // `:98-105`: the shadow's row, (8<<16)/bp + 100.
    let shadow = ((8 << 16) / i32::from(bp)) as i16;
    let shadow = shadow.wrapping_add(100);
    (x, shadow, bp)
}

/// `ASM.ASM:120-141`: the ball row after the gravity integration,
/// `(y<<6)/bp + 100`.
fn project_y(y: i16, bp: i16) -> i16 {
    let row = ((i32::from(y) << 6) / i32::from(bp)) as i16;
    row.wrapping_add(100)
}

/// `ASM.ASM:122-136`: the y integration and its floor bounce.
fn integrate(dot: &mut Dot, gravity: i16, gravityd: i16, gravitybottom: i16) {
    dot.yadd = dot.yadd.wrapping_add(gravity);
    dot.y = dot.y.wrapping_add(dot.yadd);
    if dot.y >= gravitybottom {
        // `neg ax; imul gravityd; sar ax,4` — the low word of the product,
        // arithmetic-shifted.
        let product = dot.yadd.wrapping_neg().wrapping_mul(gravityd);
        dot.yadd = product >> 4;
        dot.y = dot.y.wrapping_add(dot.yadd);
    }
}

/// Restore `bytes` bytes of `bgpic` at `at` into the video segment.
fn restore(video: &mut [u8], bgpic: &[u8], at: u16, bytes: usize) {
    let at = at as usize;
    video[at..at + bytes].copy_from_slice(&bgpic[at..at + bytes]);
}

/// Write a table entry's low `bytes` bytes at `at`.
fn write_table(video: &mut [u8], at: u16, value: u32, bytes: usize) {
    let at = at as usize;
    for i in 0..bytes {
        video[at + i] = (value >> (8 * i)) as u8;
    }
}

/// `ASM.ASM:52-218`'s `_drawdots`, unit for unit, into a 64 KB segment.
///
/// The dot order is the array order. `bx` is not carried between dots: the
/// z rotation's `sub dx,cx` ignores the carry (`bugs.md` §18.1).
///
/// The arity is the machine's: each parameter is one of the registers or
/// memory pointers the assembly reads, and bundling them would have to hold
/// the same borrows and be built at the same call site, which changes nothing.
#[allow(clippy::too_many_arguments)]
fn drawdots(
    dots: &mut [Dot],
    bgpic: &[u8],
    video: &mut [u8],
    rotcos: i32,
    rotsin: i32,
    gravity: i16,
    gravityd: i16,
    gravitybottom: i16,
    depth1: &[u32; DEPTH_ENTRIES],
    depth2: &[u32; DEPTH_ENTRIES],
    depth3: &[u32; DEPTH_ENTRIES],
) {
    for dot in dots.iter_mut() {
        let (x, shadow, bp) = project_xz(dot, rotcos, rotsin);
        let on_shadow = (x as u16) <= 319 && (shadow as u16) <= 199;
        if on_shadow {
            // `:106-118`: restore the old shadow and write the two-pixel one.
            let at = (shadow as u16)
                .wrapping_mul(320)
                .wrapping_add(x as u16);
            restore(video, bgpic, dot.old1, 2);
            video[at as usize] = SHADOW;
            video[at as usize + 1] = SHADOW;
            dot.old1 = at;

            // `:120-141`: the y integration and the ball row.
            integrate(dot, gravity, gravityd, gravitybottom);
            let row = project_y(dot.y, bp);
            if (x as u16) <= 319 && (row as u16) <= 199 {
                let at = (row as u16).wrapping_mul(320).wrapping_add(x as u16);
                // `:150-161`: restore the old three rows.
                restore(video, bgpic, dot.old2, 4);
                restore(video, bgpic, dot.old2.wrapping_add(320), 4);
                restore(video, bgpic, dot.old2.wrapping_add(640), 4);
                // `:162-177`: the depth entry and the three rows.
                let depth = (((bp as u16) >> 6) & 0xFFFC) as usize / 4;
                write_table(video, at.wrapping_add(1), depth1[depth], 2);
                write_table(video, at.wrapping_add(320), depth2[depth], 4);
                write_table(video, at.wrapping_add(641), depth3[depth], 2);
                dot.old2 = at;
            } else {
                // `@@3` (`:203-216`): the ball is off screen; restore it and
                // leave `old2` alone.
                restore(video, bgpic, dot.old2, 4);
                restore(video, bgpic, dot.old2.wrapping_add(320), 4);
                restore(video, bgpic, dot.old2.wrapping_add(640), 4);
            }
        } else {
            // `@@2` (`:185-202`): the shadow is off screen. Three 32-bit
            // restores of the old ball first (`:186`, `:189`, `:192`), each
            // `mov eax,fs:[di]` — so by `:195 mov ds:[si+6],ax` AX is the
            // **low word of the dword read from bgpic[old2+640..644]**, the
            // first two background pixels of the old ball's third row, not
            // the shadow's row. The port stores that word; `bgpic`'s bytes
            // past 64,000 are the source's undefined read and the port's zero
            // fill (D24).
            let third = dot.old2.wrapping_add(640) as usize;
            let stored = u16::from_le_bytes([bgpic[third], bgpic[third + 1]]);
            restore(video, bgpic, dot.old2, 4);
            restore(video, bgpic, dot.old2.wrapping_add(320), 4);
            restore(video, bgpic, dot.old2.wrapping_add(640), 4);
            restore(video, bgpic, dot.old1, 2);
            dot.old1 = stored;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{Music, MusicPosition};
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::view::View;
    use sr_engine::{dac_expand, Surface};

    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// The reference index that draws part tick 0 — the mapping's origin.
    const TICK_ZERO_INDEX: u32 = REFERENCE_FIRST_INDEX;

    /// A player scripted one `musplus` reading a tick.
    struct Player {
        script: Vec<i32>,
        at: usize,
        /// `poll` calls made; the entry pre-roll's 256 are what the mid-chain
        /// test proves absent.
        polls: u32,
    }

    impl Player {
        fn quiet(last_tick: u32) -> Player {
            Player {
                script: vec![0; last_tick as usize + 1],
                at: 0,
                polls: 0,
            }
        }

        fn scripted(script: Vec<i32>) -> Player {
            Player {
                script,
                at: 0,
                polls: 0,
            }
        }
    }

    impl Music for Player {
        fn poll(&mut self) {
            self.polls += 1;
        }

        fn position(&self) -> MusicPosition {
            MusicPosition::default()
        }

        fn musplus(&mut self) -> i32 {
            self.script.get(self.at).copied().unwrap_or(0)
        }

        fn musrow(&mut self) -> i32 {
            0
        }

        fn muscode(&mut self, wanted: i32) -> i32 {
            sr_engine::music::muscode_from(MusicPosition::default(), wanted)
        }

        fn sync(&mut self) -> i32 {
            unreachable!("`DOTS/MAIN.C` never syncs: its music call is `dis_musplus`")
        }

        fn getmframe(&mut self) -> i32 {
            0
        }

        fn setmframe(&mut self, _frame: i32) {}
    }

    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
        player: Player,
    }

    impl Rig {
        fn new(player: Player) -> Rig {
            Rig {
                surface: Surface::new(SURFACE_WIDTH, SURFACE_HEIGHT),
                view: View::new(SCREEN_WIDTH, SCREEN_HEIGHT),
                palette: Palette::new(),
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
                player,
            }
        }

        /// A band's context: never fresh, as the engine's band contexts are.
        fn ctx(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, false)
        }

        fn ctx_exit(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, true, false)
        }

        /// The init context of an entry-point run — the part the run's music
        /// was installed for.
        fn init_ctx(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, true)
        }

        /// The init context of a part reached mid-chain, with the player
        /// already running.
        fn init_ctx_chain(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, false)
        }

        fn ctx_with(&mut self, frame: u32, exit: bool, music_fresh: bool) -> PartCtx<'_> {
            self.player.at = frame as usize;
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(&mut self.player),
                frame,
                0,
                1,
                true,
                exit,
                music_fresh,
            )
        }
    }

    fn started_with(player: Player) -> (Rig, Minvball) {
        let mut rig = Rig::new(player);
        let mut part = Minvball::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Minvball) {
        started_with(Player::quiet(0))
    }

    /// Run `ticks` calls, asserting the part keeps going; the player's script
    /// index is the tick.
    fn run_ticks(rig: &mut Rig, part: &mut Minvball, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            assert_eq!(
                part.frame(&mut rig.ctx(at)),
                PartStatus::Continuing,
                "tick {at}"
            );
        }
    }

    fn run_to(rig: &mut Rig, part: &mut Minvball, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    /// The one-time fade, so a test can reach the main loop.
    fn run_fade(rig: &mut Rig, part: &mut Minvball) {
        run_to(rig, part, MAIN_FIRST_TICK);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(part.frame, 0);
    }

    // --- the RNG -------------------------------------------------------------

    #[test]
    fn the_rng_is_microsoft_cs_lcg_and_its_first_values_are_the_sequences() {
        // `DOTS/MAKEFILE` compiles with `cl /qc /AL` and the linked EXE
        // carries the MS run-time library's string: Microsoft C 6.0's
        // `rand()`, an LCG modulo 2^32. The first ten values of its seed 1.
        let mut rng = MsRand::new();
        let first: Vec<i32> = (0..10).map(|_| rng.next()).collect();
        assert_eq!(
            first,
            [41, 18467, 6334, 26500, 19169, 15724, 11478, 29358, 26962, 24464]
        );
        assert_eq!(MsRand::MULTIPLIER, 214_013);
        assert_eq!(MsRand::INCREMENT, 2_531_011);
        assert_eq!(MsRand::MODULUS, 1 << 32);
        // The state wraps at the modulus: the 2^32 term is dropped.
        let mut wrapped = MsRand { seed: u32::MAX };
        let _ = wrapped.next();
        assert_eq!(
            wrapped.seed,
            u32::MAX
                .wrapping_mul(MsRand::MULTIPLIER)
                .wrapping_add(MsRand::INCREMENT)
        );
    }

    #[test]
    fn the_setup_is_deterministic_and_its_permutation_is_fixed() {
        let mut rng = MsRand::new();
        let (order, dots) = Minvball::setup_dots(&mut rng);
        // The 500-swap shuffle's own first entries, from the seed-1 stream.
        assert_eq!(
            &order[..12],
            &[223, 464, 331, 287, 94, 64, 6, 173, 17, 208, 10, 216]
        );
        assert_eq!(&order[500..], &[379, 459, 500, 348, 70, 113, 475, 507, 324, 184, 258, 462]);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..DOTNUM as u16).collect::<Vec<_>>());
        // The initial positions are one value repeated, and the scramble —
        // which swaps only x, y and z — therefore moves nothing. It still
        // consumes the 1,000 calls the scatter's stream position depends on.
        for dot in &dots {
            assert_eq!((dot.x, dot.y, dot.z, dot.yadd), (0, -19440, 0, 0));
            assert_eq!((dot.old1, dot.old2), (0, 0));
        }
        // The main loop's first event reads `dottaul[88]` (`INITIAL_J`).
        assert_eq!(order[INITIAL_J], 97);

        // `setup_state` resets the RNG, so re-initializing the same instance
        // reproduces the whole deterministic setup (the shell builds a fresh
        // part per start, but the reset is what makes the seed the source's).
        let mut part = Minvball::new();
        part.setup_state(&Palette::new());
        let first_order = part.order.clone();
        let first_dots: Vec<(i16, i16, i16, i16)> = part
            .dots
            .iter()
            .map(|d| (d.x, d.y, d.z, d.yadd))
            .collect();
        part.setup_state(&Palette::new());
        assert_eq!(part.order, first_order);
        let second_dots: Vec<(i16, i16, i16, i16)> = part
            .dots
            .iter()
            .map(|d| (d.x, d.y, d.z, d.yadd))
            .collect();
        assert_eq!(second_dots, first_dots);
    }

    // --- the cue and the exit ------------------------------------------------

    #[test]
    fn the_cue_window_is_the_open_interval_minus_four_to_zero() {
        // `MAIN.C:194` `if(a>-4 && a<0) break`: -4 and 0 are *outside*.
        for (reading, fires) in [(-5, false), (-4, false), (-3, true), (-2, true), (-1, true), (0, false), (1, false)] {
            let (mut rig, mut part) = started_with(Player::quiet(1_000));
            run_fade(&mut rig, &mut part);
            // The script is indexed by the part's tick; write the reading at
            // the tick the check runs on.
            let tick = part.tick;
            rig.player.script[tick as usize] = reading;
            let status = part.frame(&mut rig.ctx(tick));
            if fires {
                assert_eq!(status, PartStatus::Finished, "musplus {reading} must break");
                assert_eq!(part.phase, Phase::Done, "musplus {reading}");
            } else {
                assert_eq!(
                    status,
                    PartStatus::Continuing,
                    "musplus {reading} is outside the window"
                );
                assert_eq!(part.phase, Phase::Main, "musplus {reading}");
            }
        }
    }

    #[test]
    fn a_skipped_cue_leaves_the_part_continuing_and_drawing() {
        // The window's two neighbours and a jump across it: the part must
        // never spin or stop.
        let mut script = vec![0i32; 1_000];
        for (tick, reading) in [(140usize, -5), (150, -4), (160, 0), (170, 1)] {
            script[tick] = reading;
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        run_to(&mut rig, &mut part, 300);
        assert_eq!(part.phase, Phase::Main);
        // It is drawing: the video is not the untouched background.
        assert_ne!(
            &rig.surface.pixels()[..VIDEO_BYTES],
            &part.bgpic[..VIDEO_BYTES]
        );
    }

    #[test]
    fn exit_requested_finishes_on_that_tick_in_both_phases() {
        // The fade.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 20);
        assert_eq!(part.phase, Phase::Fade);
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished
        );
        assert_eq!(part.phase, Phase::Done);
        // The main loop, before any cue.
        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished
        );
        assert_eq!(part.phase, Phase::Done);
    }

    #[test]
    fn the_frame_cap_ends_the_loop_at_two_thousand_four_hundred_fifty() {
        // `while(... && frame<2450)`: the last drawn frame is 2450 and the
        // next tick retires the part.
        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        // Fast-forward the physics without drawing every frame.
        while part.frame < LAST_FRAME {
            part.physics();
        }
        assert_eq!(part.frame, LAST_FRAME);
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
    }

    // --- the stages ----------------------------------------------------------

    #[test]
    fn the_stage_one_to_two_boundary_resets_f_and_starts_the_vortex() {
        // `MAIN.C:199` `if(frame==500) f=0;` and `:209-215`'s stage.
        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        while part.frame < 499 {
            part.physics();
        }
        assert_eq!(part.frame, 499);
        assert_eq!(part.f, 499);
        part.physics();
        // Frame 500 reset f to 0 and used the stage-2 formula on this dot.
        assert_eq!(part.frame, 500);
        assert_eq!(part.f, 1, "f was reset and then incremented");
        let first = part.order[(INITIAL_J + 499) % DOTNUM] as usize;
        assert_eq!(first, 75);
        let dot = part.dots[first];
        assert_eq!(dot.x, (icos(0) * 55) as i16);
        assert_eq!(dot.y, part.dropper);
        assert_eq!(dot.z, (isin(0) * 55) as i16);
        assert_eq!(dot.yadd, -260);
        // The dropper has reached its floor (`:270`).
        assert_eq!(part.dropper, DROPPER_FLOOR);
    }

    #[test]
    fn the_gravity_decrements_at_the_thirty_second_frame_after_nineteen_hundred() {
        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        while part.frame < 1919 {
            part.physics();
        }
        assert_eq!(part.grav, GRAV_START);
        part.physics(); // 1920
        assert_eq!(part.grav, GRAV_START - 1);
        while part.frame < 1951 {
            part.physics();
        }
        assert_eq!(part.grav, GRAV_START - 1);
        part.physics(); // 1952
        assert_eq!(part.grav, GRAV_START - 2);
    }

    #[test]
    fn the_depth_tables_are_the_sources_ramp() {
        let (d1, d2, d3) = Minvball::build_depth_tables();
        // c = clamp((a-31)*3/4 + 8, 0, 15), then 15-c.
        assert_eq!(d1[0], 0x0202u32.wrapping_add(0x0404_0404 * 15));
        assert_eq!(d1[40], 0x0202u32.wrapping_add(0x0404_0404));
        assert_eq!(d1[127], 0x0202);
        assert_eq!(d2[0], 0x0203_0302u32.wrapping_add(0x0404_0404 * 15));
        assert_eq!(d3, d1, "depthtable3 is depthtable1's twin");
    }

    #[test]
    fn the_palette_is_the_sources_and_the_dac_starts_black() {
        let (rig, part) = started();
        let built = Minvball::build_palette();
        assert_eq!(&part.pal[..64 * 3], &built[..64 * 3]);
        assert_eq!(&part.pal[64 * 3..164 * 3], &built[64 * 3..164 * 3]);
        assert_eq!(&part.pal[255 * 3..255 * 3 + 3], &[31, 0, 15]);
        // `:151` zeroes the DAC after the read-back.
        assert!((0..=255u8).all(|i| rig.palette.entry(i) == [0, 0, 0]));
        // The floor's ramp: row 150 carries index 114, whose value is 14.
        assert_eq!(part.video[150 * 320], 114);
        assert_eq!(part.bgpic[150 * 320], 114);
        assert!(part.video[..100 * 320].iter().all(|p| *p == 0));
        // The shadow's colour is inside the background's ramp, not outside
        // it: entry 87 is a grey whose six-bit value is 11, widened to 45.
        assert_eq!(built[87 * 3], built[87 * 3 + 1]);
        assert_eq!(built[87 * 3], built[87 * 3 + 2]);
        assert_eq!(built[87 * 3], 11);
        assert_eq!(dac_expand(built[87 * 3]), 45);
    }

    // --- the drawing ---------------------------------------------------------

    #[test]
    fn the_shadow_restore_bug_stores_the_old_balls_third_row_word() {
        // A dot whose x is off screen: the three 32-bit restores at
        // `ASM.ASM:186-193` leave EAX holding the dword at bgpic[old2+640],
        // and `:195 mov ds:[si+6],ax` stores its low word. At rot = 128 (45°)
        // the projection is 345, outside 0..319.
        let dot = Dot {
            x: 16384,
            z: 16384,
            ..Dot::default()
        };
        let (x, shadow, bp) = project_xz(&dot, 11584, 11584);
        assert_eq!((x, shadow, bp), (345, 158, 9000));
        let mut dots = [dot];
        let mut bgpic = vec![0u8; FRAME_MEMORY];
        // The old ball's third row (old2 = 0, so bgpic[640..]) carries a
        // marker word: that word is what the bug stores.
        bgpic[640] = 0x34;
        bgpic[641] = 0x12;
        bgpic[642] = 0x99;
        bgpic[643] = 0x88;
        // And a second marker at the shadow's would-be row, to prove it is
        // *not* what is stored.
        bgpic[158] = 0xAA;
        bgpic[159] = 0xBB;
        let mut video = vec![0u8; FRAME_MEMORY];
        let d1 = [0u32; DEPTH_ENTRIES];
        let d2 = [0u32; DEPTH_ENTRIES];
        let d3 = [0u32; DEPTH_ENTRIES];
        drawdots(
            &mut dots, &bgpic, &mut video, 11584, 11584, 3, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        assert_eq!(
            dots[0].old1, 0x1234,
            "the low word of bgpic[old2+640..644], not the shadow row"
        );
        assert_eq!(dots[0].old2, 0, "the off-screen ball keeps old2");
        // The next frame restores at that word as a pixel offset and reads
        // the background there; a marker at 0x1234 proves the offset is used.
        bgpic[0x1234] = 0xCC;
        bgpic[0x1235] = 0xDD;
        dots[0].x = 0;
        dots[0].z = 0;
        drawdots(
            &mut dots, &bgpic, &mut video, 16384, 0, 3, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        assert_eq!(
            &video[0x1234..0x1236],
            &[0xCC, 0xDD],
            "the restore used the stored word as an offset"
        );
    }

    /// `bugs.md` §2: shadows and balls are painted in the dot array's own
    /// order and there is no depth test, so a later (farther) dot's shadow is
    /// drawn over an earlier (nearer) dot's ball, and swapping the order swaps
    /// which one shows. Sorting the dots by depth, or drawing every shadow
    /// first, would put the near ball on top in both orders.
    #[test]
    fn shadows_are_drawn_over_balls_without_a_depth_test() {
        // Rotation 0: cos 16384, sin 0, so x' = X and bp = Z/4 + 9000.
        let (cos, sin) = (16384, 0);
        // The near dot: Z = -4000, bp 8000; y = 7125 puts its ball's rows at
        // 157, 158 and 159 with the wide middle row (four pixels) at 158,
        // columns 160..=163.
        let near = Dot {
            x: 0,
            y: 7125,
            z: -4000,
            ..Dot::default()
        };
        // The far dot: Z = 0, bp 9000;
        // its two-pixel shadow lands on row 158, columns 160 and 161.
        let far = Dot {
            x: 0,
            y: 0,
            z: 0,
            ..Dot::default()
        };
        let (near_x, near_shadow, near_bp) = project_xz(&near, cos, sin);
        assert_eq!((near_x, near_shadow, near_bp), (160, 165, 8000));
        assert_eq!(project_y(near.y, near_bp), 157, "the near ball starts on row 157");
        let (far_x, far_shadow, far_bp) = project_xz(&far, cos, sin);
        assert_eq!((far_x, far_shadow), (160, 158));
        assert!(far_bp > near_bp, "the far dot is farther than the near one");

        let (d1, d2, d3) = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        let paint = |order: [Dot; 2]| -> Vec<u8> {
            let mut dots = order;
            let mut video = vec![0u8; FRAME_MEMORY];
            drawdots(
                &mut dots, &bgpic, &mut video, cos, sin, 0, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
            );
            video
        };
        let middle = 158 * 320 + 160;
        let near_depth = (((near_bp as u16) >> 6) & 0xFFFC) as usize / 4;
        let ball = d2[near_depth].to_le_bytes();
        assert!(
            !ball.contains(&SHADOW),
            "the ball's own bytes differ from the shadow colour, so the two can be told apart"
        );

        // Near first, far second: the far dot's shadow is drawn over the near
        // dot's ball, although the ball is nearer.
        let video = paint([near, far]);
        assert_eq!(
            &video[middle..middle + 4],
            &[SHADOW, SHADOW, ball[2], ball[3]],
            "the later, farther shadow covers the first two pixels of the nearer ball"
        );
        // Far first, near second: the near ball is drawn over the far shadow.
        let video = paint([far, near]);
        assert_eq!(
            &video[middle..middle + 4],
            &ball,
            "the later, nearer ball covers the earlier shadow: the draw order alone decides"
        );
    }

    /// `bugs.md` §18.1: the z rotation is `sub ax,bx` / `sub dx,cx`
    /// (`DOTS/ASM.ASM:69-70`), and `sub dx,cx` ignores the carry, so the low
    /// word's borrow never reaches `bp`; `ax` is overwritten at `:74`, which
    /// makes the stale `bx` (and the `mov ax,ax` at `:65`) inert:
    /// `bp = hi(Z*cos) - hi(X*sin) + 9000`, whatever the previous dot left.
    #[test]
    fn the_stale_bx_never_reaches_bp() {
        // Z = -3228 at rotation 0: Z*cos is -807 * 65536, whose low word is 0,
        // the case where a borrow from a larger `bx` would move bp by one and
        // the shadow row from 163 to 164.
        let dot = Dot {
            x: 0,
            y: 0,
            z: -3228,
            ..Dot::default()
        };
        assert_eq!(
            project_xz(&dot, 16384, 0),
            (160, 163, 8193),
            "bp = -807 + 9000: the low word's borrow never reaches it"
        );

        // Through `drawdots`: a dot drawn after another one (whose ball offset
        // is the register the source leaves in `bx`) casts the same shadow as
        // the same dot drawn alone.
        let (d1, d2, d3) = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        let mut alone = vec![0u8; FRAME_MEMORY];
        drawdots(
            &mut [dot], &bgpic, &mut alone, 16384, 0, 0, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        let mut after = vec![0u8; FRAME_MEMORY];
        let mut dots = [Dot::default(), dot];
        drawdots(
            &mut dots, &bgpic, &mut after, 16384, 0, 0, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        assert_eq!(after[163 * 320 + 160], SHADOW, "the shadow is on row 163 either way");
        assert_eq!(alone[163 * 320 + 160], SHADOW);
        assert_eq!(after[164 * 320 + 160], 0);
    }

    /// `bugs.md` §18.3: the gravity integration is inside the ball block and
    /// `ja @@2` (`DOTS/ASM.ASM:96`, `:105`) jumps past it when the shadow is
    /// off screen, so such a dot's `y` and `yadd` hold still.
    #[test]
    fn an_off_screen_shadow_holds_the_dots_y_still() {
        let (d1, d2, d3) = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        // At rotation 45 degrees this dot's x projects to 345, off screen.
        let off = Dot {
            x: 16384,
            y: 100,
            z: 16384,
            yadd: 7,
            ..Dot::default()
        };
        let (x, _, _) = project_xz(&off, 11584, 11584);
        assert!(x as u16 > 319, "the shadow is off screen");
        let mut dots = [off];
        let mut video = vec![0u8; FRAME_MEMORY];
        drawdots(
            &mut dots, &bgpic, &mut video, 11584, 11584, 3, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        assert_eq!((dots[0].y, dots[0].yadd), (100, 7), "gravity is skipped");
        // The same dot on screen integrates: yadd += gravity, then y += yadd.
        let on = Dot {
            x: 0,
            y: 100,
            z: 0,
            yadd: 7,
            ..Dot::default()
        };
        let mut dots = [on];
        drawdots(
            &mut dots, &bgpic, &mut video, 16384, 0, 3, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        assert_eq!((dots[0].yadd, dots[0].y), (10, 110), "gravity runs on screen");
    }

    #[test]
    fn the_ball_rows_are_the_three_depth_table_writes() {
        // A dot at the origin projects to x=160, shadow row ~158, ball row
        // ~100; the three rows are the tables' bytes at bx+1, bx+320, bx+641.
        let mut dots = [Dot {
            y: 0,
            ..Dot::default()
        }];
        let bgpic = vec![0u8; FRAME_MEMORY];
        let mut video = vec![0u8; FRAME_MEMORY];
        let (d1, d2, d3) = Minvball::build_depth_tables();
        drawdots(
            &mut dots, &bgpic, &mut video, 16384, 0, 0, 13, GRAVITY_BOTTOM, &d1, &d2, &d3,
        );
        let dot = dots[0];
        // bp = 9000; x = 160, shadow row = 58+100 = 158, ball row = 0+100.
        let (x, shadow, bp) = project_xz(&dot, 16384, 0);
        assert_eq!((x, shadow), (160, 158));
        let row = project_y(dot.y, bp);
        let at = (row as u16) * 320 + x as u16;
        assert_eq!((row, at), (100, 32_160));
        let depth = (((bp as u16) >> 6) & 0xFFFC) as usize / 4;
        let word = d1[depth] as u16;
        assert_eq!(&video[at as usize + 1..at as usize + 3], &word.to_le_bytes());
        let dword = d2[depth];
        assert_eq!(
            &video[at as usize + 320..at as usize + 324],
            &dword.to_le_bytes()
        );
        let word = d3[depth] as u16;
        assert_eq!(
            &video[at as usize + 641..at as usize + 643],
            &word.to_le_bytes()
        );
        assert_eq!(dot.old2, at);
    }

    #[test]
    fn the_music_pre_roll_puts_the_reference_state_at_tick_zero() {
        // The trace at boot 10,368 (the part's tick 0) reads order 75 row 61,
        // `zplus` 1 -> `musplus` -3. Seeked to 75, the player's 257th poll is
        // that state, so 256 pre-polls and the engine's tick-0 poll land it.
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        assert_eq!(MUSIC_PRE_ROLL, 256);
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(75);
        let mut music = PlayerMusic::new(player.clone(), 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        // The engine's tick-0 poll.
        music.poll();
        let position = music.position();
        assert_eq!((position.order, position.row), (75, 61));
        assert_eq!(lock(&player).zplus(), 1);
        assert_eq!(music.musplus(), -3);
        // The fade's 130 ticks: the main loop's first tick reads order 77,
        // the trace's position at boot 10,498.
        for _ in 0..MAIN_FIRST_TICK {
            music.poll();
        }
        let position = music.position();
        assert_eq!(position.order, 77);
        assert!((28..=30).contains(&position.row), "row {}", position.row);
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        // A part the block's chain reaches is handed a player that has
        // already run through this stretch; 256 more polls would replay it.
        // The pre-roll is the entry run's (`music_fresh`) alone.
        let mut rig = Rig::new(Player::quiet(0));
        let mut part = Minvball::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player.polls, 0, "a mid-chain start polls nothing");
    }

    // --- the uninitialized `j`, and the entry register -----------------------

    /// The RGB bytes the shell would compose from the part's video buffer and
    /// its `pal` (the frames measured here are before the 2,300 palette
    /// uploads, so `pal` is the live palette).
    fn rendered_rgb(part: &Minvball) -> Vec<u8> {
        let mut out = Vec::with_capacity(VIDEO_BYTES * 3);
        for byte in &part.video[..VIDEO_BYTES] {
            let at = *byte as usize * 3;
            out.push(dac_expand(part.pal[at]));
            out.push(dac_expand(part.pal[at + 1]));
            out.push(dac_expand(part.pal[at + 2]));
        }
        out
    }

    /// Replay the main loop from frame 1 to `frame`, drawing every tick, with
    /// the cursor at `j`. The fade is not replayed: it
    /// draws nothing, and the reference frames are main-loop frames.
    fn replay(j: usize, frame: i32) -> Vec<u8> {
        let mut part = Minvball::new();
        part.setup_state(&Palette::new());
        part.next = j;
        for _ in 1..=frame {
            part.physics();
            part.draw_video();
        }
        rendered_rgb(&part)
    }

    #[test]
    #[ignore = "measurement: the 512-value j fit; run with --release --ignored --nocapture"]
    fn the_initial_j_is_fitted_to_the_reference() {
        // The source never assigns `j` (`MAIN.C:87`, `:200`). The reference's
        // 10,900 frame is the evidence: replay it under each of the 512
        // cursors and assert the port's value is the minimum. This is the
        // command the note and A19 name for the fitted number.
        let reference =
            std::fs::read("../../reference/parts/minvball/frame_010900.rgb").unwrap();
        assert_eq!(reference.len(), VIDEO_BYTES * 3);
        let mut scores: Vec<(usize, usize)> = (0..DOTNUM)
            .map(|j| {
                let rgb = replay(j, 403);
                let diff = rgb
                    .chunks_exact(3)
                    .zip(reference.chunks_exact(3))
                    .filter(|(a, b)| a != b)
                    .count();
                (diff, j)
            })
            .collect();
        scores.sort_unstable();
        println!("best five (pixels, j): {:?}", &scores[..5]);
        assert_eq!(
            scores[0].1, INITIAL_J,
            "the fitted cursor is the minimum over 0..512"
        );
        // Unique, not a near-tie: 88 reproduces the frame exactly and the
        // next best cursor leaves 283 pixels (`bugs.md` §18.4, A19).
        assert_eq!(
            (scores[0].0, scores[1].0),
            (0, 283),
            "88 is the only cursor with no residue; the runner-up's pixels"
        );
    }

    // --- the reference's mapping --------------------------------------------

    #[test]
    fn the_mapping_is_boot_equals_ten_thousand_three_hundred_sixty_eight_plus_tick() {
        // Tick 0 is the fade's first wait; the main loop starts at tick 130
        // and draws frame counter 1 at boot 10,498.
        assert_eq!(REFERENCE_FIRST_INDEX, 10_368);
        assert_eq!(MAIN_FIRST_TICK, 130);
        assert_eq!(MAIN_FIRST_FRAME_INDEX, 10_498);
        assert_eq!(TICK_ZERO_INDEX, 10_368);
        assert_eq!(MAIN_FIRST_FRAME_INDEX, REFERENCE_FIRST_INDEX + MAIN_FIRST_TICK);
        // index = 10,368 + tick and frame = index - 10,497. The survey's
        // 12,900 frame is therefore frame counter 2403, inside the
        // 2400-2439 every-entry fade.
        assert_eq!(12_900 - 10_497, 2_403);
        for (index, tick, frame) in [
            (10_900u32, 532u32, 403i32),
            (11_200, 832, 703),
            (11_800, 1432, 1303),
            (12_400, 2032, 1903),
        ] {
            assert_eq!(index - REFERENCE_FIRST_INDEX, tick);
            assert_eq!(index as i32 - (REFERENCE_FIRST_INDEX as i32 + MAIN_FIRST_TICK as i32 - 1), frame);
        }
    }

    #[test]
    fn the_repeat_difference_is_zero_at_the_keyframes() {
        // The frame-counter differences equal the boot differences: every
        // iteration between 10,900 and 12,400 took one vblank, so the
        // original's `repeat` was 1 throughout and the port's one-tick model
        // loses nothing there. (The y integration — one a frame — also has
        // to match for the keyframes to match; `tests/minvball.rs` measures
        // it against the reference.)
        let frames = [403i32, 703, 1303, 1903];
        let ticks = [532u32, 832, 1432, 2032];
        for window in 0..frames.len() - 1 {
            assert_eq!(
                frames[window + 1] - frames[window],
                (ticks[window + 1] - ticks[window]) as i32,
                "window {window}"
            );
        }
        assert_eq!(frames[1] - frames[0], 300);
        assert_eq!(frames[2] - frames[1], 600);
        assert_eq!(frames[3] - frames[2], 600);
    }

    #[test]
    fn the_fade_uploads_on_its_odd_ticks_and_ends_with_the_full_palette() {
        // Tick 0: no upload. Tick 1: `pal - 64` (all black). Tick 129:
        // `pal - 0`, the live palette, and the main loop starts at 130.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 1);
        assert!((0..=255u8).all(|i| rig.palette.entry(i) == [0, 0, 0]));
        run_to(&mut rig, &mut part, 2);
        // `pal - 64` is zero for every entry the part sets (all <= 63).
        assert!((0..=255u8).all(|i| rig.palette.entry(i) == [0, 0, 0]));
        run_to(&mut rig, &mut part, 130);
        assert_eq!(part.phase, Phase::Main);
        for entry in 0..=163usize {
            assert_eq!(
                rig.palette.entry(entry as u8),
                [
                    part.pal[entry * 3],
                    part.pal[entry * 3 + 1],
                    part.pal[entry * 3 + 2]
                ],
                "entry {entry} at the last upload"
            );
        }
    }
}

