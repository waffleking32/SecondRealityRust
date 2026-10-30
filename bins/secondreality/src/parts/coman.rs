//! COMAN — the 3D sinus field (`3dsinfld`).
//!
//! `COMAN/MAIN.C` (228 lines) over `COMAN/ASM.ASM` (`inittwk`, `docol`,
//! `docopy`, `setpalarea`) and `COMAN/THELOOP.INC` — the 5,230-line
//! unrolled ray march `COMAN/DOLOOP.C` generates — with the two linked wave
//! tables `W1DTA.BIN`/`W2DTA.BIN`.
//!
//! # The screen: mode 13h with chain-4 off
//!
//! `inittwk` (`ASM.ASM:36-78`) leaves CRTC `0x14 = 0` and `0x17 = 0xE3`,
//! takes the sequencer's memory mode to 6 (**chain-4 off**) and the map mask
//! to `0x0F`, and never writes CRTC `0x13`, so the row offset stays mode
//! 13h's 40 words = **80 plane bytes a row**. In the emulator's unchained
//! handler a CPU byte address is four plane bytes (`VGA_UnchainedVGA_Handler`
//! stores at `4*addr + plane`), and the display reads them straight back as
//! four consecutive pixels (`M_VGA`'s line is one linear byte a pixel, 320
//! bytes a scanline). `docopy` writes the vbuf row's first 80 bytes under map
//! mask `0x03` (planes 0 and 1) and its second 80 under `0x0C` (planes 2 and
//! 3), so every displayed pair of pixels repeats one vbuf byte: the 160-wide
//! buffer becomes the 320-wide picture with each column shown twice.
//!
//! # The parts
//!
//! 1. **Init** (`MAIN.C:118-227`): the palette formula — `uc = (223 - a*22/26)*3`
//!    walks entries 223 down to 8, the green `(230-a)/4 + sin1024[a*4]/32`,
//!    the blue `(255-a)/3` and the red diamond `40-|a-220|`, all `/3`, then
//!    the `*9/6` brightness over bytes 0..719 (entries 0..239), the dark-red
//!    bar at 232..255, entry 0 black, and the `combg` read at 240..255 whose
//!    array is 16 bytes and whose indexes are 736..783 (see "The unlinked
//!    background"). `halloc(16384,4)` is the 64 KB vbuf, `CALCW12` is off so
//!    the wave tables are the linked `W1DTA`/`W2DTA`, and the video memory is
//!    cleared.
//! 2. **`doit`** (`MAIN.C:45-116`): the music gate (`musplus < 0`), then
//!    iterations of `frame < 4444`: the `a > -8 && a < 0` break, the
//!    `frepeat` catch-up that raises `startrise` near an order's end and
//!    lowers it for the first 400 frames, the sine camera
//!    (`rot2 += 4; rot += sin1024[rot2&1023]/15; r = rot>>3` and the four
//!    sines at `r`, `r+256`, `r+177`, `r+433`), the 160-column `docol`, the
//!    two-page flip (`0x0C = 0x80`/`0x00`) and `docopy`.
//! 3. **`theloop`** (`THELOOP.INC`): the swapped 16.16 ray march, unit for
//!    unit in [`theloop`].
//!
//! # The tick model, and the measured mapping
//!
//! The original's `dis_waitb` returns the **number of copper frames** that
//! elapsed since the previous call (`DIS/DISINT.ASM`'s INDEMO arm reads and
//! clears `copperframecount`). One render iteration costs two of them: the
//! reference's picture changes every second boot frame (measured at all four
//! keyframes, `reference-indices.md` §16.3), and `frame += (frepeat =
//! dis_waitb())` therefore advances the source's counter by 2. The port runs
//! one engine tick a vblank: an iteration runs on one tick, the next is two
//! ticks later, and the picture the engine composes on the intervening tick
//! is the same page — the source is blocked inside `dis_waitb`. The one
//! exception is the frame-4 palette upload, whose two `dis_waitb` calls in
//! the inner body each spend a vblank (see [`Coman::iteration`]): the
//! iteration that starts with `frame == 4` runs at tick 4 and schedules the
//! next at tick 8, so the iteration ticks are `0, 2, 4, 8, 10, ...`.
//!
//! A render's picture is copied into the page the **next** iteration
//! selects, so the beam shows picture `k` from the tick iteration `k+1` runs
//! — two ticks after it was drawn, and for the two ticks of that iteration's
//! wait. The reference pins the result: at a `--part 3dsinfld` run's ticks
//! **464/465, 966/967, 1470/1471 and 1772/1773** the port is byte-identical
//! to reference frames **15500, 16000, 16500 and 16800** (the process test
//! `tests/coman.rs`). The implied origins drift — 15,036, 15,034, 15,030 and
//! 15,028 — because the reference renders at about 1.99 frames an iteration
//! (its `dis_waitb` sometimes returns one), while the port's model is a fixed
//! two; the mapping is stated per keyframe rather than as a boot anchor.
//! The gate opens at tick 0, so that tick is the order-93 row-0 crossing the
//! block-3 trace puts at 15031–15038.
//!
//! # The entry, and the music
//!
//! `shell::music_for_part("3dsinfld")` seeks **MUSIC1 order 91** — the order
//! the trace holds across the part's start window's beginning. The row is
//! *derived, not measured*: the window `(15000, 15100]` spans the 91→93
//! crossing (order 92 is the `254` marker) and the part marks no start
//! frame. [`Coman::MUSIC_PRE_ROLL`] pre-polls 527 frames, the polls between
//! `_zplaysong(91)`'s row 0 and the gate: the 527th leaves the published row
//! at 63 with `musplus == -1`, the engine's first tick polls the 528th and
//! sees order 93 row 0, `musplus == 0`. A mid-chain start polls nothing
//! (`PartCtx::music_fresh`).
//!
//! The part ends on `MAIN.C:59-60`'s break, `a > -8 && a < 0`, in MUSIC1
//! order 96's `zplus == 1` tail: the published row 57 first gives
//! `musplus == -7`, and the iterating part reads it at tick 2,014. The
//! 4,444-frame cap is never reached. In that tail the inner body also raises
//! `startrise` a vblank. Once it reaches 130 the copy's first row
//! `70 + startrise` is at or below row 200 and the screen is flat black —
//! the reference's 17,000, before the part itself ends.
//!
//! # The unlinked background
//!
//! `COMBG.UH`, `COMBG.X` and `COMBG.LBM` are **not in the link**: `MAKEFILE`
//! links `main.obj asm.obj theloop.obj disc.obj` plus the four wave-table
//! objects, and `asm.obj`'s `combguse` is the only `combg` name in it. The
//! C global is `char combg[16]` (`MAIN.C:8`), so every `combg` index the
//! source uses is an **overrun**:
//!
//! - the palette's `palette[x] = combg[16+x]` for `x = 720..767` reads
//!   `combg[736..783]`;
//! - the `combguse` fill (`:166-176`) reads up to `combg[29582]`.
//!
//! `combguse` is read only by `docopy`'s `IFDEF PXLSUX` half (`ASM.ASM:143-172`),
//! and `MAKEFILE`'s `tasm $(asm_f)` defines no `PXLSUX`, so that half is not
//! assembled and **there is no background**. The overrun's destination —
//! DAC entries 240..255 — is unreachable too: the ray march's colour is
//! `((bx + 140 - j/8) & 0xFF) >> 1`, never above 127. The port therefore
//! keeps the 16-byte array, models the palette read as an overrun that cannot
//! be observed, and does not draw the dead background. The tests pin both
//! facts.
//!
//! # The preserved bugs
//!
//! - The `combg` overrun and the dead `combguse` background, above.
//! - The palette aliasing: the `*9/6` pass brightens entries 0..239, the bar
//!   writes 232..255, the `combg` overrun then overwrites 240..255, and
//!   entry 0 is forced black.
//! - The frame-4 `setpalarea` is dead: it re-uploads the same palette.
//! - The `frame < 4444` cap is dead on this boot: the music break ends the
//!   part at about 1,900 frames.
//! - `cameralevel` (`MAIN.C:54`) is written and read by `theloop` only into
//!   `theloop_heigth`, which the generated code never reads.
//! - `docol` masks **all four** coordinate arguments with `~1` (`ASM.ASM:115-123`).
//! - `docopy`'s first clear is 18 rows at destination row 52, and `rep stosd`
//!   advances `di` over them: the 140-row copy starts at destination row
//!   **70**, not 52. The clear stays visible above the picture. The second
//!   clear (vbuf rows 68..97) lands after the copy and so is seen by the
//!   *next* frame.
//! - The wave tables' 16-bit index wrap is their 64 KB length; `si`/`di` wrap
//!   and the word read masks the low bit.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

/// The part's registry name: what `--part` accepts.
const NAME: &str = "3dsinfld";

// ---------------------------------------------------------------- assets --

/// The linked X wave table (`MAIN.C:212` `wave1 = w1dta`).
const W1_PATH: &str = "assets/COMAN/W1DTA.BIN";
/// The linked Y wave table.
const W2_PATH: &str = "assets/COMAN/W2DTA.BIN";
/// A wave table is 32,768 signed words, and `fs:[si]`/`gs:[di]` wrap at 64 KB:
/// the byte offset is the 16-bit register and the word read masks its low bit.
const WAVE_BYTES: usize = 65_536;

// --------------------------------------------------------------- screens --

const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// Mode 13h's row stride and this part's: 80 plane bytes = 320 pixels.
const LINE_LEN: u16 = 320;
/// `halloc(16384,4)`: the 64 KB vbuf, addressed through a 16-bit `bp`.
const VBUF_BYTES: usize = 65_536;
/// The unchained VGA's plane address space: 64 K addresses, four plane bytes
/// each (`VGA_UnchainedVGA_Handler`'s `((uint32_t*)linear)[addr]`).
const PLANE_BYTES: usize = 65_536;
/// The linear video memory the plane addresses name: 256 KB.
const VIDEO_BYTES: usize = 4 * PLANE_BYTES;
/// The two pages the flip toggles: segment `a000` = plane 0, `a800` = plane
/// `0x8000` (the handler masks the physical address with `0xFFFF`).
const PAGE_LO: u16 = 0x0000;
const PAGE_HI: u16 = 0x8000;
/// `docopy` clears 18 rows at destination row 52 — and `rep stosd` advances
/// `di` over them — before copying 140 rows from vbuf row 60 to destination
/// row **70** + `startrise`.
const COPY_ROWS: usize = 140;
const COPY_SOURCE_ROW: usize = 60;
const CLEAR_DEST_ROW: usize = 52;
const CLEAR_DEST_ROWS: usize = 18;
const COPY_DEST_ROW: usize = CLEAR_DEST_ROW + CLEAR_DEST_ROWS;
/// `docopy`'s post-copy clear: vbuf rows 68..97 (30 rows), `cx = 30*160/4/4`.
const CLEAR_FIRST_ROW: usize = 68;
const CLEAR_ROWS: usize = 30;

// ---------------------------------------------------------------- tables --

/// `MAIN.C:85-98`: the field of view.
const COLUMNS: i32 = 160;
/// `MAIN.C:48`: the rise counter's initial and maximum value.
const STARTRISE_MAX: i32 = 160;
/// `MAIN.C:57`: the dead frame cap.
const FRAME_CAP: u32 = 4444;
/// `MAIN.C:52`, `:57`: the counter starts at 0, and the music break is
/// `a > -8 && a < 0`.
const BREAK_LOW: i32 = -8;
/// `MAIN.C:63`: `startrise` rises while `a > -30 && a < 0`.
const RISE_LOW: i32 = -30;

/// One render iteration's vblanks: the measured two-frame step. The
/// source's `dis_waitb` returns the elapsed copper frames and the reference's
/// picture changes every second boot frame.
const VBLANKS_PER_ITERATION: u32 = 2;

/// `DOLOOP.C`: `bail = 192`, the number of ray steps; `bailhalve = 64`, where
/// the horizontal step doubles. The generated labels are `0..63` and then
/// every second index to 190.
const BAIL: usize = 192;
const BAIL_HALVE: usize = 64;
/// The start of the direction: `firstraydir = -(200-horizony)*2560` with
/// `horizony = 70`, swapped by `wflip` (`THELOOP.INC:42`).
const FIRST_RAY_DIR: u32 = 0xEC00_FFFA;

/// `DOLOOP.C`'s `wflip`: the low and high 16-bit halves trade places.
///
/// The ray state is 16.16 with the **integer part in the low word** and the
/// fraction in the high one, so a 32-bit `add` carries the wrong way; the
/// generated code's `adc ax,0` / `adc ax,-1` fixes the carry out of bit 31
/// back into the integer half.
pub fn wflip(l: u32) -> u32 {
    ((l & 0xFFFF) << 16).wrapping_add(l >> 16)
}

/// `THELOOP.INC`'s `zwave[j] = 16*sin(2*pi*3*j/192)`, truncated to an integer
/// by the generator's `int` assignment. Only the labels the shipped code
/// actually emits are used: every `j < 64`, then every second `j >= 64`.
pub fn zwave(j: usize) -> i16 {
    let value = 16.0 * (j as f64 * std::f64::consts::PI * 2.0 * 3.0 / BAIL as f64).sin();
    value as i16
}

/// The generated `_@seekj` label set (`DOLOOP.C:89-160`): `0..64`, then
/// `64, 66, ..., 190`. Label 192 is the `ret`.
pub fn labels() -> impl Iterator<Item = usize> {
    (0..BAIL_HALVE).chain((BAIL_HALVE..BAIL).step_by(2))
}

/// `COMAN/SIN1024.INC`, 1,024 signed words scaled to 256 — its own table, not
/// the one `TECHNO/KOEA.ASM` carries, though the two files' bytes agree.
pub const SIN1024: [i16; 1024] = [
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

/// The 16-byte `combg` the palette and `combguse` read (`MAIN.C:8`). The
/// shipped link carries no such asset; every index the source uses is beyond
/// this array (see the module's "The unlinked background").
const COMBG: [u8; 16] = [0; 16];

/// The 48 bytes the palette reads out of `combg[736..784]` — the overrun.
///
/// The C global's neighbours are the next declarations in `MAIN.C`
/// (`flip`, then the 1,024-word `wavesin`), but the compiler's data order is
/// not part of any evidence here, and **the bytes cannot be observed**: the
/// pinhole colour is at most 127 and the `PXLSUX` background is not in the
/// link. The port keeps the read as an explicit out-of-bounds model and makes
/// the unobservability a test.
fn combg_overrun() -> [u8; 48] {
    // `MAIN.C:161-164`: `for(x=(256-16)*3; x<768; x++) palette[x] =
    // combg[16+x];` — indexes 16+720 .. 16+767, i.e. 736..783, of a 16-byte
    // array.
    let mut out = [0u8; 48];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = COMBG.get(16 + 720 + i).copied().unwrap_or(0);
    }
    out
}

/// `MAIN.C:127-164`: the palette, six bits a component.
///
/// Entries 223 down to 8 come from the formula, then the `*9/6` brightness
/// over entries 0..239, then the dark-red bar at 232..239, entry 0 black, and
/// the `combg` overrun at 240..255 last.
pub fn palette() -> Palette {
    let mut bytes = [0u8; 768];
    for a in 0..256i32 {
        let uc = (223 - a * 22 / 26) * 3;
        let mut b = (230 - a) / 4;
        b += i32::from(SIN1024[((a * 4) & 1023) as usize]) / 32;
        let b = b.clamp(0, 63);
        bytes[uc as usize + 1] = b as u8;
        let b = ((255 - a) / 3).min(63);
        bytes[uc as usize + 2] = b as u8;
        let mut b = a - 220;
        if b < 0 {
            b = -b;
        }
        if b > 40 {
            b = 40;
        }
        b = 40 - b;
        bytes[uc as usize] = (b / 3) as u8;
    }
    for byte in bytes.iter_mut().take(720) {
        let b = (*byte as i32 * 9 / 6).min(63);
        *byte = b as u8;
    }
    for a in 0..24i32 {
        let uc = (255 - a) * 3;
        let b = (a - 4).max(0) / 2;
        bytes[uc as usize] = b as u8;
        bytes[uc as usize + 1] = 0;
        bytes[uc as usize + 2] = 0;
    }
    bytes[0] = 0;
    bytes[1] = 0;
    bytes[2] = 0;
    bytes[720..768].copy_from_slice(&combg_overrun());

    let mut palette = Palette::new();
    for (entry, rgb) in bytes.chunks_exact(3).enumerate() {
        palette.set_entry(entry as u8, [rgb[0], rgb[1], rgb[2]]);
    }
    palette
}

// --------------------------------------------------------------- theloop --

/// The wave table word at byte offset `at`: `fs:[si]`, whose 16-bit index
/// wraps at 64 KB and whose low bit is not part of the address.
fn wave_word(table: &[u8], at: u16) -> i16 {
    let at = (at & 0xFFFE) as usize;
    i16::from_le_bytes([table[at], table[at + 1]])
}

/// One `add` of the swapped 16.16 form, with the carry out of bit 31.
fn add32(a: u32, b: u32) -> (u32, bool) {
    let sum = a.wrapping_add(b);
    (sum, sum < a)
}

/// `adc ax,imm`: the 16-bit add's own carry is discarded, as on the 8086.
fn adc_ax(ax: u16, imm: u16, carry: bool) -> u16 {
    ax.wrapping_add(imm).wrapping_add(u16::from(carry))
}

/// `adc ecx,imm; adc cx,0`: a 32-bit add with a carry in, and the carry out
/// lands back in the integer half.
fn adc_ecx(ecx: &mut u32, imm: u32, carry_in: bool) -> bool {
    let (sum, carry) = add32(*ecx, imm.wrapping_add(u32::from(carry_in)));
    let cx = adc_ax(sum as u16, 0, carry);
    *ecx = (sum & 0xFFFF_0000) | u32::from(cx);
    carry
}

/// The whole of `THELOOP.INC`, step for step.
///
/// `si`/`di` are the wave byte offsets, `xsina`/`ysina` the even-masked
/// column directions, and `bp` the vbuf offset. The march is unrolled per
/// label: `j = 0..63` steps the wave by `xsina`, then `j = 64, 66, ..., 190`
/// steps it by `2*xsina` and adds the ray direction twice. Every write goes
/// through a 16-bit `bp` into the 64 KB segment.
#[allow(clippy::too_many_arguments)]
pub fn theloop(
    vbuf: &mut [u8; VBUF_BYTES],
    wave1: &[u8],
    wave2: &[u8],
    si: u16,
    di: u16,
    xsina: u16,
    ysina: u16,
    mut bp: u16,
) {
    let mut si = si;
    let mut di = di;
    let xsina1 = xsina;
    let xsina2 = xsina.wrapping_mul(2);
    let ysina1 = ysina;
    let ysina2 = ysina.wrapping_mul(2);
    let mut eax: u32 = 0;
    let mut ecx: u32 = FIRST_RAY_DIR;

    /// `add eax, value; adc ax, addend` — the 32-bit add, its low word, then
    /// the carry out of bit 31 back into the integer half, which the next
    /// `add eax` reads.
    fn add_eax(eax: &mut u32, value: u32, addend: u16) -> u16 {
        let (sum, carry) = add32(*eax, value);
        let ax = adc_ax(sum as u16, addend, carry);
        *eax = (sum & 0xFFFF_0000) | u32::from(ax);
        ax
    }

    for j in labels() {
        let (xs, ys) = if j < BAIL_HALVE {
            (xsina1, ysina1)
        } else {
            (xsina2, ysina2)
        };
        si = si.wrapping_add(xs);
        let mut bx = wave_word(wave1, si) as u16;
        di = di.wrapping_add(ys);
        bx = bx.wrapping_add(wave_word(wave2, di) as u16);
        bx = bx.wrapping_add((zwave(j) as i32 - 240) as u16);
        let mut ax = eax as u16;
        if (ax as i16) < (bx as i16) {
            let dx = bx.wrapping_add((140 - j / 8) as u16);
            let dl = (dx as u8) >> 1;
            let l = wflip((j * 2560) as u32);
            loop {
                ax = add_eax(&mut eax, l, 0);
                vbuf[bp as usize] = dl;
                if (ax as i16) >= (bx as i16) {
                    // `_@seek1a`: the `adc ecx` carries the **compare's** CF,
                    // which a taken signed `jge` does not zero (0 >= -1 is
                    // true, and unsigned 0 < 65535).
                    let cf = ax < bx;
                    adc_ecx(&mut ecx, 0x0A00_0000, cf);
                    bp = bp.wrapping_sub(160);
                    break;
                }
                ax = add_eax(&mut eax, l, 0);
                vbuf[bp.wrapping_sub(160) as usize] = dl;
                if (ax as i16) >= (bx as i16) {
                    // `_@seek2a`: a bare `add ecx`, then the `_@seek1a` `adc`.
                    bp = bp.wrapping_sub(160);
                    let (sum, carry) = add32(ecx, 0x0A00_0000);
                    ecx = sum;
                    adc_ecx(&mut ecx, 0x0A00_0000, carry);
                    bp = bp.wrapping_sub(160);
                    break;
                }
                ax = add_eax(&mut eax, l, 0);
                vbuf[bp.wrapping_sub(320) as usize] = dl;
                adc_ecx(&mut ecx, 0x1E00_0000, false);
                bp = bp.wrapping_sub(480);
                if (ax as i16) >= (bx as i16) {
                    break;
                }
                // `jmp _@hitj`
            }
        }
        // `_@seeko`
        let (sum, mut carry) = add32(eax, ecx);
        eax = sum;
        if j >= BAIL_HALVE {
            let (sum, carry2) = add32(eax, ecx.wrapping_add(u32::from(carry)));
            eax = sum;
            carry = carry2;
        }
        ax = adc_ax(eax as u16, 0xFFFF, carry);
        eax = (eax & 0xFFFF_0000) | u32::from(ax);
    }
    let _ = (eax, ecx, bp);
}

/// `docol` (`ASM.ASM:105-132`): mask all four coordinate arguments with ~1
/// and call the march. `b` (the vbuf offset) is passed through unmasked.
/// The source's `ax = cameralevel` lands in `theloop_heigth`, which no
/// emitted instruction reads; the port does not carry it.
#[allow(clippy::too_many_arguments)]
pub fn docol(
    vbuf: &mut [u8; VBUF_BYTES],
    wave1: &[u8],
    wave2: &[u8],
    xw: i32,
    yw: i32,
    xa: i32,
    ya: i32,
    b: i32,
) {
    let mask = |v: i32| (v as u16) & 0xFFFE;
    theloop(
        vbuf,
        wave1,
        wave2,
        mask(xw),
        mask(yw),
        mask(xa),
        mask(ya),
        b as u16,
    );
}

/// `docopy` (`ASM.ASM:134-236`), the live half.
///
/// The 18-row `rep stosd` clear at destination row 52 advances `di` over
/// itself, so the 140 rows of vbuf 60..199 reach the page at row
/// **70 + startrise**; each 80-byte half of a vbuf row lands in one plane
/// pair at the same address, which the display reads as two pixels each. The
/// `PXLSUX` half is not assembled; the post-copy clear of vbuf rows 68..97
/// is.
pub fn docopy(vbuf: &mut [u8; VBUF_BYTES], video: &mut [u8; VIDEO_BYTES], page: u16, startrise: i32) {
    let base = (u32::from(page).wrapping_add((80 * startrise) as u32)) & 0xFFFF;
    // `ASM.ASM:178-182`: `di = 52*80`, `cx = 18*80/4`, `rep stosd` with all
    // planes masked — 18 rows of index 0, and `di` ends 18 rows on.
    for at in 0..CLEAR_DEST_ROWS * 80 {
        let dest = ((base + (CLEAR_DEST_ROW * 80 + at) as u32) & 0xFFFF) as usize;
        video[4 * dest..4 * dest + 4].fill(0);
    }
    for row in 0..COPY_ROWS {
        let source = (COPY_SOURCE_ROW + row) * 160;
        for k in 0..80 {
            let dest = ((base + (COPY_DEST_ROW + row) as u32 * 80 + k as u32) & 0xFFFF) as usize;
            let a = vbuf[source + k];
            let b = vbuf[source + 80 + k];
            let at = 4 * dest;
            video[at] = a;
            video[at + 1] = a;
            video[at + 2] = b;
            video[at + 3] = b;
        }
    }
    // `ASM.ASM:216-229`: vbuf rows 68..97, 30 rows, cleared after the copy.
    let start = CLEAR_FIRST_ROW * 160;
    vbuf[start..start + CLEAR_ROWS * 160].fill(0);
}

/// The displayed picture from `page`: `M_VGA`'s line reads 320 linear bytes a
/// scanline (`vga.draw.address_add = scan_len*8`), and the plane address is
/// four bytes into the unchained memory.
fn compose(video: &[u8; VIDEO_BYTES], page: u16, surface: &mut [u8]) {
    let base = usize::from(page) * 4;
    let rows = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
    surface[..rows].copy_from_slice(&video[base..base + rows]);
}

// ------------------------------------------------------------------ part --

/// The COMAN sinus field.
pub struct Coman {
    /// The part's asset root, so the unit tests can read `../../assets`.
    root: &'static str,
    wave1: Vec<u8>,
    wave2: Vec<u8>,
    vbuf: Box<[u8; VBUF_BYTES]>,
    video: Box<[u8; VIDEO_BYTES]>,
    /// The camera's sine cursor and accumulated rotation (`MAIN.C:50-53`).
    rot2: i32,
    rot: i32,
    /// The wave cursors, advanced once at `a == 80` (`MAIN.C:93-97`).
    xwav: i32,
    ywav: i32,
    /// `startrise`: the copy's vertical shift, 0..160.
    startrise: i32,
    /// The source's `frame` counter, advanced by `dis_waitb`'s return.
    frame: u32,
    /// The inner body's iteration count for the next iteration
    /// (`while(frepeat--)`).
    frepeat: i32,
    /// `MAIN.C`'s `flip`: 0 or 1, toggled every iteration.
    flip: u8,
    /// The CRTC 0x0C start the last iteration selected: 0 or `0x8000`.
    start_page: u16,
    /// The part tick the next iteration runs on.
    next_iteration: u32,
    /// The gate has passed.
    started: bool,
    /// The source's loop has broken or the frame cap reached.
    done: bool,
    palette: Palette,
}

impl Coman {
    /// A fresh part reading `assets/COMAN/...` from the repository root.
    pub fn new() -> Coman {
        Coman {
            root: "",
            wave1: Vec::new(),
            wave2: Vec::new(),
            vbuf: Box::new([0; VBUF_BYTES]),
            video: Box::new([0; VIDEO_BYTES]),
            rot2: 0,
            rot: 0,
            xwav: 0,
            ywav: 0,
            startrise: STARTRISE_MAX,
            frame: 0,
            frepeat: 1,
            flip: 0,
            start_page: PAGE_LO,
            next_iteration: 0,
            started: false,
            done: false,
            palette: Palette::new(),
        }
    }

    #[cfg(test)]
    fn at(root: &'static str) -> Coman {
        Coman {
            root,
            ..Coman::new()
        }
    }

    fn asset(&self, path: &str) -> String {
        format!("{}{path}", self.root)
    }

    /// Read one wave table; a short or absent file is a part error.
    fn read_wave(&self, path: &str) -> Result<Vec<u8>, PartError> {
        let full = self.asset(path);
        let bytes = std::fs::read(&full).map_err(|error| PartError {
            part: NAME,
            message: format!("{full}: {error}"),
        })?;
        if bytes.len() != WAVE_BYTES {
            return Err(PartError {
                part: NAME,
                message: format!("{full} is {} bytes; the linked table is {WAVE_BYTES}", bytes.len()),
            });
        }
        Ok(bytes)
    }

    /// `MAIN.C:85-98`: one iteration's camera and 160 columns.
    fn render(&mut self) {
        self.rot2 = self.rot2.wrapping_add(4);
        self.rot += i32::from(SIN1024[(self.rot2 & 1023) as usize]) / 15;
        let r = self.rot >> 3;
        let rsin = i32::from(SIN1024[(r & 1023) as usize]);
        let rcos = i32::from(SIN1024[((r + 256) & 1023) as usize]);
        let rsin2 = i32::from(SIN1024[((r + 177) & 1023) as usize]);
        let rcos2 = i32::from(SIN1024[((r + 177 + 256) & 1023) as usize]);
        // `xw`/`yw` are copies made before the loop and are **not** updated
        // in it: `xwav`/`ywav` change at `a == 80` for the *next* frame.
        let xw = self.xwav;
        let yw = self.ywav;
        for a in 0..COLUMNS {
            let x = a - 80;
            let y = 160;
            let xa = (x * rcos + y * rsin) / 256;
            let ya = (y * rcos2 - x * rsin2) / 256;
            let b = (a & 1) * 80 + (a >> 1) + 199 * 160;
            docol(
                &mut self.vbuf,
                &self.wave1,
                &self.wave2,
                xw,
                yw,
                xa,
                ya,
                b,
            );
            if a == 80 {
                self.xwav += xa * 4;
                self.ywav += ya * 4;
            }
        }
    }

    /// The page the last iteration copied into, which is the picture just
    /// rendered.
    #[cfg(test)]
    fn flip_page(&self) -> u16 {
        if self.flip == 0 { PAGE_LO } else { PAGE_HI }
    }

    /// One main-loop iteration (`MAIN.C:57-114`) with the music reading `a`:
    /// the `frepeat` inner bodies, the render, the flip and the copy.
    fn iteration(&mut self, tick: u32, a: i32) {
        let mut n = self.frepeat;
        self.frepeat = -1;
        let mut extra_waits = 0u32;
        while n > 0 {
            if a > RISE_LOW && a < 0 && self.startrise < STARTRISE_MAX {
                self.startrise += 1;
            }
            if self.frame < 400 && self.startrise > 0 {
                if self.frame == 4 {
                    // `MAIN.C:69-73`: `dis_waitb(); setpalarea(...)` — the
                    // upload is the palette already in force, and the wait
                    // spends a vblank.
                    extra_waits += 1;
                }
                self.startrise -= 1;
            }
            n -= 1;
        }
        self.render();
        self.flip += 1;
        if self.flip > 1 {
            self.flip = 0;
        }
        let (start, copy) = if self.flip == 0 {
            (PAGE_HI, PAGE_LO)
        } else {
            (PAGE_LO, PAGE_HI)
        };
        self.start_page = start;
        self.frame += VBLANKS_PER_ITERATION;
        self.frepeat = VBLANKS_PER_ITERATION as i32;
        docopy(&mut self.vbuf, &mut self.video, copy, self.startrise);
        self.next_iteration = tick + VBLANKS_PER_ITERATION + extra_waits;
    }
}

impl Default for Coman {
    fn default() -> Self {
        Coman::new()
    }
}

impl Part for Coman {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part opens with a music gate (`COMAN/MAIN.C:56` \
             `while(!dis_exit() && dis_musplus()<0)`) and the run has no music",
        )?;
        self.wave1 = self.read_wave(W1_PATH)?;
        self.wave2 = self.read_wave(W2_PATH)?;
        self.palette = palette();
        ctx.set_palette(self.palette);
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = SCREEN_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(LINE_LEN);
        ctx.view.set_origin(0);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
        // `mov ax,13h; int 10h` and `inittwk`'s 64 KB clear leave the video
        // memory (and so the screen) black before `doit`'s gate; without this
        // a chain would show the previous part's picture through the gate.
        compose(&self.video, PAGE_LO, ctx.surface.pixels_mut());
        if ctx.music_fresh() {
            for _ in 0..Self::MUSIC_PRE_ROLL {
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
        let a = ctx.musplus();
        if !self.started {
            if a < 0 {
                return PartStatus::Continuing;
            }
            self.started = true;
            self.next_iteration = ctx.frame;
        } else if ctx.frame < self.next_iteration {
            // `dis_waitb` is still counting the elapsed frames.
            return PartStatus::Continuing;
        }
        if self.frame >= FRAME_CAP {
            self.done = true;
            return PartStatus::Finished;
        }
        if a > BREAK_LOW && a < 0 {
            self.done = true;
            return PartStatus::Finished;
        }
        self.iteration(ctx.frame, a);
        compose(&self.video, self.start_page, ctx.surface.pixels_mut());
        PartStatus::Continuing
    }
}

impl Coman {
    /// The entry pre-roll: the polls between `_zplaysong(91)`'s row 0 and the
    /// reference's position when the part's gate opens.
    pub(crate) const MUSIC_PRE_ROLL: u32 = 527;
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::music::Music;
    use sr_engine::view::View;

    #[test]
    fn wflip_swaps_the_halves() {
        assert_eq!(wflip(0xFFFA_EC00), 0xEC00_FFFA);
        assert_eq!(wflip(0), 0);
        assert_eq!(wflip(0x0000_0A00), 0x0A00_0000);
        assert_eq!(wflip(0x0000_0001), 0x0001_0000);
        // `-(200-70)*2560` is `0xFFFAEC00`; the source's `mov ecx,0EC00FFFAh`.
        assert_eq!(wflip((-(130i32 * 2560)) as u32), FIRST_RAY_DIR);
    }

    #[test]
    fn the_sine_table_is_the_sources_own_1024_words() {
        let text = std::fs::read_to_string("../../upstream/SecondReality/COMAN/SIN1024.INC")
            .expect("SIN1024.INC");
        let mut words = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("dw") {
                for word in rest.split(',') {
                    words.push(word.trim().parse::<i16>().unwrap());
                }
            }
        }
        assert_eq!(words.len(), 1024);
        assert_eq!(&words[..], &SIN1024[..]);
    }

    /// A tiny interpreter for the shipped `THELOOP.INC`, so the march can be
    /// tested against the **source file** rather than against a second
    /// reading of it.
    mod generated {
        use super::super::{wave_word, wflip, zwave, FIRST_RAY_DIR};
        use std::collections::HashMap;

        #[derive(Debug, Clone)]
        enum Op {
            AddSi(usize),
            MovBxFs,
            AddDi(usize),
            AddBxGs,
            AddBxImm(i32),
            Cmp,
            Jge(String),
            LeaDx(i32),
            ShrDl,
            AddEax(u32),
            AddEaxEcx,
            AdcEaxEcx,
            AdcAx(u16),
            MovEsDl(i32),
            AddEcx(u32),
            AdcEcx(u32),
            AdcCx0,
            SubBp(u16),
            XorEax,
            MovEcx(u32),
            Jmp(String),
        }

        #[derive(Debug, Default)]
        pub struct Program {
            ops: Vec<Op>,
            labels: HashMap<String, usize>,
        }

        fn hex(text: &str) -> u32 {
            let text = text.trim().trim_end_matches('h');
            u32::from_str_radix(text, 16)
                .unwrap_or_else(|e| panic!("bad hex {text:?} ({e})"))
        }

        pub fn parse(text: &str) -> Program {
            let mut program = Program::default();
            let mut inside = false;
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with("theloop PROC") {
                    inside = true;
                    continue;
                }
                if line == "theloop ENDP" || line == "ret" {
                    if inside {
                        let at = program.ops.len();
                        program.labels.insert(format!("ret{}", at), at);
                        program.ops.push(Op::Jmp("__ret".into()));
                    }
                    continue;
                }
                if !inside || line.is_empty() || line.starts_with(';') {
                    continue;
                }
                if let Some(label) = line.strip_suffix(':') {
                    program.labels.insert(label.to_string(), program.ops.len());
                    continue;
                }
                if line.starts_with("mov cs:")
                    || line.starts_with("mov ax,cs")
                    || line.starts_with("mov ds,ax")
                    || line == "add cx,cx"
                    || line == "add dx,dx"
                {
                    continue;
                }
                let op = if line.starts_with("xor eax,eax") {
                    Op::XorEax
                } else if let Some(rest) = line.strip_prefix("mov ecx,") {
                    Op::MovEcx(hex(rest))
                } else if let Some(rest) = line.strip_prefix("add si,ds:theloop_xsina") {
                    Op::AddSi(rest.parse().unwrap())
                } else if line.starts_with("mov bx,fs:[si]") {
                    Op::MovBxFs
                } else if let Some(rest) = line.strip_prefix("add di,ds:theloop_ysina") {
                    Op::AddDi(rest.parse().unwrap())
                } else if line.starts_with("add bx,gs:[di]") {
                    Op::AddBxGs
                } else if let Some(rest) = line.strip_prefix("add bx,") {
                    Op::AddBxImm(rest.parse().unwrap())
                } else if line == "cmp ax,bx" {
                    Op::Cmp
                } else if let Some(rest) = line.strip_prefix("jge short ") {
                    Op::Jge(rest.to_string())
                } else if let Some(rest) = line.strip_prefix("lea dx,[bx+") {
                    Op::LeaDx(rest.trim_end_matches(']').parse().unwrap())
                } else if line == "shr dl,1" {
                    Op::ShrDl
                } else if line == "add eax,ecx" {
                    Op::AddEaxEcx
                } else if line == "adc eax,ecx" {
                    Op::AdcEaxEcx
                } else if let Some(rest) = line.strip_prefix("add eax,") {
                    Op::AddEax(hex(rest))
                } else if let Some(rest) = line.strip_prefix("adc ax,") {
                    Op::AdcAx(rest.parse::<i32>().unwrap() as u16)
                } else if line == "mov es:[bp],dl" {
                    Op::MovEsDl(0)
                } else if line == "mov es:[bp-160],dl" {
                    Op::MovEsDl(-160)
                } else if line == "mov es:[bp-320],dl" {
                    Op::MovEsDl(-320)
                } else if let Some(rest) = line.strip_prefix("add ecx,") {
                    Op::AddEcx(hex(rest))
                } else if let Some(rest) = line.strip_prefix("adc ecx,") {
                    Op::AdcEcx(hex(rest))
                } else if line.starts_with("adc cx,0") {
                    Op::AdcCx0
                } else if let Some(rest) = line.strip_prefix("sub bp,") {
                    Op::SubBp(rest.replace("160*3", "480").parse().unwrap())
                } else if let Some(rest) = line.strip_prefix("jmp ") {
                    Op::Jmp(rest.to_string())
                } else {
                    panic!("uninterpreted line: {line}");
                };
                program.ops.push(op);
            }
            program
        }

        /// Run the program from its first instruction. The setup lines are
        /// the entry contract (`mov cs:theloop_xsina1,cx`, ...): `cx`/`dx`
        /// hold the masked directions and `ax` the unused camera height.
        #[allow(clippy::too_many_arguments)]
        pub fn run(
            program: &Program,
            vbuf: &mut [u8; 65536],
            wave1: &[u8],
            wave2: &[u8],
            mut si: u16,
            mut di: u16,
            mut xsina1: u16,
            mut ysina1: u16,
            mut bp: u16,
            camera: u16,
        ) {
            let xsina2 = xsina1.wrapping_mul(2);
            let ysina2 = ysina1.wrapping_mul(2);
            xsina1 &= 0xfffe;
            ysina1 &= 0xfffe;
            let _ = camera;
            let mut eax: u32 = 0;
            let mut ecx: u32 = FIRST_RAY_DIR;
            let mut bx: u16 = 0;
            let mut dx: u16 = 0;
            let mut cf = false;
            let mut pc = 0usize;
            let mut steps = 0usize;
            while pc < program.ops.len() {
                steps += 1;
                assert!(steps < 1_000_000, "the generated program did not return");
                let mut next = pc + 1;
                match &program.ops[pc] {
                    Op::AddSi(which) => {
                        si = si.wrapping_add(if *which == 1 { xsina1 } else { xsina2 });
                    }
                    Op::MovBxFs => bx = wave_word(wave1, si) as u16,
                    Op::AddDi(which) => {
                        di = di.wrapping_add(if *which == 1 { ysina1 } else { ysina2 });
                    }
                    Op::AddBxGs => bx = bx.wrapping_add(wave_word(wave2, di) as u16),
                    Op::AddBxImm(value) => bx = bx.wrapping_add(*value as u16),
                    Op::Cmp => {
                        let ax = eax as u16;
                        cf = ax < bx;
                    }
                    Op::Jge(label) => {
                        let ax = eax as i16;
                        if ax >= bx as i16 {
                            next = program.labels[label];
                        }
                    }
                    Op::LeaDx(value) => dx = bx.wrapping_add(*value as u16),
                    Op::ShrDl => dx = (dx & 0xFF00) | ((dx as u8) >> 1) as u16,
                    Op::AddEax(value) => {
                        let sum = eax.wrapping_add(*value);
                        cf = sum < eax;
                        eax = sum;
                    }
                    Op::AddEaxEcx => {
                        let sum = eax.wrapping_add(ecx);
                        cf = sum < eax;
                        eax = sum;
                    }
                    Op::AdcEaxEcx => {
                        let sum = eax.wrapping_add(ecx).wrapping_add(u32::from(cf));
                        cf = sum < eax;
                        eax = sum;
                    }
                    Op::AdcAx(value) => {
                        let ax = (eax as u16).wrapping_add(*value).wrapping_add(u16::from(cf));
                        eax = (eax & 0xFFFF_0000) | u32::from(ax);
                    }
                    Op::MovEsDl(offset) => {
                        vbuf[bp.wrapping_add(*offset as u16) as usize] = dx as u8;
                    }
                    Op::AddEcx(value) => {
                        let sum = ecx.wrapping_add(*value);
                        cf = sum < ecx;
                        ecx = sum;
                    }
                    Op::AdcEcx(value) => {
                        let sum = ecx.wrapping_add(*value).wrapping_add(u32::from(cf));
                        cf = sum < ecx;
                        ecx = sum;
                    }
                    Op::AdcCx0 => {
                        let cx = (ecx as u16).wrapping_add(u16::from(cf));
                        ecx = (ecx & 0xFFFF_0000) | u32::from(cx);
                    }
                    Op::SubBp(value) => bp = bp.wrapping_sub(*value),
                    Op::XorEax => eax = 0,
                    Op::MovEcx(value) => ecx = *value,
                    Op::Jmp(label) => {
                        if label == "__ret" {
                            return;
                        }
                        next = program.labels[label];
                    }
                }
                pc = next;
                let _ = wflip;
                let _ = zwave;
            }
        }
    }

    fn read_waves() -> (Vec<u8>, Vec<u8>) {
        (
            std::fs::read("../../assets/COMAN/W1DTA.BIN").unwrap(),
            std::fs::read("../../assets/COMAN/W2DTA.BIN").unwrap(),
        )
    }

    #[test]
    fn the_ray_march_matches_the_shipped_generated_program() {
        let text = std::fs::read_to_string("../../upstream/SecondReality/COMAN/THELOOP.INC")
            .expect("THELOOP.INC");
        let program = generated::parse(&text);
        let (wave1, wave2) = read_waves();
        // The first column of the first iteration and three other columns,
        // with the camera at `rot2 = 4` (`rot = 0`, `r = 0`).
        let rsin = 0i32;
        let rcos = 256i32;
        let rsin2 = i32::from(super::SIN1024[177]);
        let rcos2 = i32::from(super::SIN1024[433]);
        for a in [0i32, 1, 79, 80, 159] {
            let x = a - 80;
            let y = 160;
            let xa = (x * rcos + y * rsin) / 256;
            let ya = (y * rcos2 - x * rsin2) / 256;
            let b = (a & 1) * 80 + (a >> 1) + 199 * 160;
            let mask = |v: i32| (v as u16) & 0xFFFE;
            let mut mine = Box::new([0u8; VBUF_BYTES]);
            docol(&mut mine, &wave1, &wave2, 0, 0, xa, ya, b);
            let mut theirs = Box::new([0u8; VBUF_BYTES]);
            generated::run(
                &program,
                &mut theirs,
                &wave1,
                &wave2,
                mask(0),
                mask(0),
                mask(xa),
                mask(ya),
                b as u16,
                0,
            );
            assert_eq!(
                &mine[..],
                &theirs[..],
                "column a = {a} is not the shipped program's"
            );
            assert!(
                mine.iter().any(|&byte| byte != 0),
                "column a = {a} drew nothing"
            );
        }
    }

    #[test]
    fn the_zwave_is_the_generators_three_sines_at_the_labels_it_emits() {
        let text = std::fs::read_to_string("../../upstream/SecondReality/COMAN/THELOOP.INC")
            .expect("THELOOP.INC");
        let mut at: Option<usize> = None;
        let mut seen = 0;
        for line in text.lines() {
            if let Some(rest) = line.trim().strip_prefix("_@seek") {
                let label = rest.strip_suffix(':').unwrap_or(rest);
                if !label.is_empty() && label.bytes().all(|b| b.is_ascii_digit()) {
                    at = Some(label.parse().unwrap());
                }
                continue;
            }
            if let (Some(j), Some(rest)) = (at, line.trim().strip_prefix("add bx,")) {
                if j < BAIL {
                    let Ok(constant) = rest.parse::<i32>() else {
                        continue;
                    };
                    assert_eq!(
                        zwave(j) as i32 - 240,
                        constant,
                        "zwave[{j}] against THELOOP.INC's `add bx,{constant}`"
                    );
                    seen += 1;
                    at = None;
                }
            }
        }
        assert_eq!(seen, 128, "every emitted label carries its zwave constant");
    }
    // ---------------------------------------------------------- the screen --

    /// A vbuf with a distinct value at every byte, so a copy that moves a
    /// byte or a plane is visible.
    fn patterned_vbuf() -> Box<[u8; VBUF_BYTES]> {
        let mut vbuf = Box::new([0u8; VBUF_BYTES]);
        for (i, byte) in vbuf.iter_mut().enumerate() {
            *byte = (i % 251) as u8 + 1;
        }
        vbuf
    }

    #[test]
    fn docopy_doubles_each_column_and_starts_below_its_eighteen_row_clear() {
        let mut vbuf = patterned_vbuf();
        let source = vbuf.clone();
        let mut video = Box::new([0u8; VIDEO_BYTES]);
        docopy(&mut vbuf, &mut video, PAGE_LO, 0);

        // The `rep stosd` clear covers destination rows 52..69 and leaves `di`
        // at row 70; the copy starts there, not at 52.
        for row in 52..70usize {
            for k in 0..80usize {
                let at = 4 * (row * 80 + k);
                assert_eq!(
                    &video[at..at + 4],
                    &[0, 0, 0, 0],
                    "row {row} is not the 18-row clear"
                );
            }
        }
        // Row 70 takes vbuf row 60: the first half to planes 0 and 1, the
        // second to planes 2 and 3, so the display's four pixels are
        // `(a, a, b, b)` — each of the 160 columns shown twice.
        for row in 0..140usize {
            for k in 0..80usize {
                let a = source[(60 + row) * 160 + k];
                let b = source[(60 + row) * 160 + 80 + k];
                let at = 4 * ((70 + row) * 80 + k);
                assert_eq!(&video[at..at + 4], &[a, a, b, b], "row {row} k {k}");
            }
        }
        // `docopy`'s final clear: vbuf rows 68..97, seen by the next frame.
        for row in 60..200usize {
            for k in [0usize, 79, 80, 159] {
                let at = row * 160 + k;
                let expected = if (68..98).contains(&row) { 0 } else { source[at] };
                assert_eq!(vbuf[at], expected, "vbuf row {row} k {k}");
            }
        }
        // The dead `PXLSUX` half would have filled rows 0..59 (the background
        // is not in the link); nothing writes them.
        assert!(video[..4 * 52 * 80].iter().all(|&b| b == 0));
    }

    #[test]
    fn docopy_shifts_by_one_row_a_startrise_unit() {
        let mut vbuf = patterned_vbuf();
        let mut video = Box::new([0u8; VIDEO_BYTES]);
        docopy(&mut vbuf, &mut video, PAGE_HI, 8);
        // `5*startrise` is a segment offset: 5*8*16 = 640 bytes = 80 plane
        // addresses = one displayed row a `startrise` unit.
        let source = patterned_vbuf();
        let at = 4 * (PAGE_HI as usize + (8 * 80) + 70 * 80);
        assert_eq!(
            &video[at..at + 4],
            &[source[60 * 160], source[60 * 160], source[60 * 160 + 80], source[60 * 160 + 80]]
        );
        // And the clear moved with it.
        let clear = 4 * (PAGE_HI as usize + 8 * 80 + 52 * 80);
        assert_eq!(&video[clear..clear + 4], &[0, 0, 0, 0]);
    }

    #[test]
    fn the_palette_is_the_formula_with_the_bar_and_the_unreachable_overrun() {
        let pal = palette();
        assert_eq!(pal.entry(0), [0, 0, 0], "entry 0 forced black");
        assert_eq!(pal.entry(1), [0, 0, 0]);
        // a = 255 -> entry 8: red (40-35)/3 = 1, green and blue clamped 0.
        assert_eq!(pal.entry(8), [1, 0, 0]);
        // a = 220 -> entry 37: the red diamond's peak, 40/3 then *9/6 = 19;
        // blue (255-220)/3 = 11, then *9/6 = 16; green
        // (230-220)/4 + sin[880]/32 = 2 - 6 = -4, clamped 0.
        assert_eq!(pal.entry(37), [19, 0, 16]);
        // a = 146 -> entry 100: green (230-146)/4 + sin[584]/32 = 21 - 3 =
        // 18, blue (255-146)/3 = 36, red 0; brightness gives 27 and 54.
        assert_eq!(pal.entry(100), [0, 27, 54]);
        // a = 0 -> entry 223: the formula's top, brightness-scaled.
        assert_eq!(pal.entry(223), [0, 63, 63]);
        // Entries 224..231 are never written by the formula or the bar: black.
        for entry in 224..232u8 {
            assert_eq!(pal.entry(entry), [0, 0, 0], "entry {entry}");
        }
        // The dark-red bar: entries 255 down to 232, red (a-4)/2 — but the
        // `combg` overrun immediately overwrites 240..255, so only 232..239
        // survive.
        for a in 16..24i32 {
            let entry = (255 - a) as u8;
            assert_eq!(pal.entry(entry), [((a - 4).max(0) / 2) as u8, 0, 0], "entry {entry}");
        }
        // Entries 240..255 are the `combg[736..784]` overrun; the array is 16
        // bytes and the shipped link has no such asset. The port's model is
        // zeros, and no screen pixel can reach them: the pinhole is at most
        // 127.
        for entry in 240..=255u8 {
            assert_eq!(pal.entry(entry), [0, 0, 0], "the overrun at {entry}");
        }
        // The pinhole colour is `((bx + 140 - j/8) & 0xFF) >> 1`: never
        // above 127, whatever the wave tables and `zwave` produce.
        let mut max_colour = 0u8;
        for bx in i16::MIN..=i16::MAX {
            for j in 0..BAIL {
                let colour = ((bx as u16).wrapping_add((140 - j / 8) as u16) as u8) >> 1;
                max_colour = max_colour.max(colour);
            }
        }
        assert_eq!(max_colour, 127);
        assert!(max_colour <= 127, "the terrain cannot reach entries 128..255");
    }

    // ------------------------------------------------------------ the part --

    /// A test rig: one surface, the live palette, the engine's timeline and
    /// copper, and a real player.
    struct Rig {
        surface: sr_engine::Surface,
        view: View,
        palette: Palette,
        copper: sr_engine::copper::CopperRegistry,
        music: crate::audio::PlayerMusic,
        frame: u32,
    }

    impl Rig {
        fn new() -> Rig {
            use crate::audio::{lock, test_player, PlayerMusic};
            use crate::shell::ModuleId;
            let player = test_player(ModuleId::Music1).unwrap();
            lock(&player).play_song(91);
            Rig {
                surface: sr_engine::Surface::new(sr_engine::MAX_WIDTH, sr_engine::MAX_HEIGHT),
                view: View::new(SCREEN_WIDTH, SCREEN_HEIGHT),
                palette: Palette::new(),
                copper: sr_engine::copper::CopperRegistry::new(),
                music: PlayerMusic::new(player, 0),
                frame: 0,
            }
        }

        fn init(&mut self, part: &mut Coman, fresh: bool) -> Result<(), PartError> {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.copper,
                Some(&mut self.music),
                self.frame,
                0,
                1,
                false,
                fresh,
            );
            part.init(&mut ctx)
        }

        fn tick(&mut self, part: &mut Coman, exit: bool) -> PartStatus {
            use sr_engine::music::Music;
            self.music.poll();
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.copper,
                Some(&mut self.music),
                self.frame,
                0,
                1,
                exit,
                false,
            );
            self.frame += 1;
            part.frame(&mut ctx)
        }
    }

    #[test]
    fn the_entry_pre_roll_lands_the_gate_on_order_93_row_zero() {
        use sr_engine::music::Music;
        let mut rig = Rig::new();
        // The player is fresh from `_zplaysong(91)`: no poll has advanced it.
        assert_eq!(rig.music.position().order, 0);
        let mut part = Coman::at("../../");
        rig.init(&mut part, true).unwrap();
        let position = rig.music.position();
        assert_eq!(position.order, 91);
        assert_eq!(position.row, 63, "527 polls leave the published row at 63");
        assert_eq!(rig.music.musplus(), -1, "the gate has not opened yet");
        // The engine's first tick polls once more: the 528th poll is the
        // order-93 row-0 crossing the reference's gate opens on.
        rig.music.poll();
        let position = rig.music.position();
        assert_eq!((position.order, position.row), (93, 0));
        assert_eq!(rig.music.musplus(), 0);
        assert_eq!(Coman::MUSIC_PRE_ROLL, 527);
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_poll_and_blacks_the_inherited_screen() {
        let mut rig = Rig::new();
        rig.surface.pixels_mut().fill(0xFF);
        let mut part = Coman::at("../../");
        rig.init(&mut part, false).unwrap();
        assert_eq!(rig.music.position().order, 0, "nothing was polled");
        // The mode set and `inittwk` clear the screen the previous part left.
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
    }

    #[test]
    fn the_iterations_run_every_second_tick_and_the_frame_four_waits_add_two() {
        // The source's `frame += (frepeat = dis_waitb())`: two vblanks an
        // iteration, and the frame-4 body's two `dis_waitb` calls add two.
        let mut part = Coman::at("../../");
        part.wave1 = part.read_wave(W1_PATH).unwrap();
        part.wave2 = part.read_wave(W2_PATH).unwrap();
        part.started = true;
        part.next_iteration = 0;
        let mut ticks = Vec::new();
        for tick in 0..24u32 {
            if tick == part.next_iteration {
                ticks.push(tick);
                part.iteration(tick, -32);
            }
        }
        assert_eq!(ticks, [0, 2, 4, 8, 10, 12, 14, 16, 18, 20, 22]);
        // The source's own counter advances 2 an iteration, including the
        // frame-4 one: 0, 2, 4 (the frame-4 iteration), 6, ...
        assert_eq!(part.frame, 22);
        // The first iteration's inner body runs once (`frepeat` starts at 1)
        // and every later one twice: 1 + 10*2 = 21 decrements.
        assert_eq!(part.startrise, STARTRISE_MAX - 21, "one decrement a vblank");
    }

    #[test]
    fn the_two_pages_alternate_and_the_display_lags_one_iteration() {
        let mut part = Coman::at("../../");
        part.wave1 = part.read_wave(W1_PATH).unwrap();
        part.wave2 = part.read_wave(W2_PATH).unwrap();
        let mut last_copy: Option<u16> = None;
        for k in 0..6usize {
            part.iteration(k as u32 * 2, -32);
            let copy = part.flip_page();
            // The start the iteration wrote names the page the previous
            // iteration copied into — the one the beam shows now.
            if let Some(previous) = last_copy {
                assert_eq!(part.start_page, previous, "iteration {k}'s displayed page");
            } else {
                assert_eq!(part.start_page, PAGE_LO, "the first frame shows page 0");
            }
            assert_ne!(part.start_page, copy, "the copy is the other page");
            last_copy = Some(copy);
        }
    }

    #[test]
    fn the_gate_opens_on_the_first_tick_and_the_first_picture_lags_one_iteration() {
        let mut rig = Rig::new();
        let mut part = Coman::at("../../");
        rig.init(&mut part, true).unwrap();
        // The engine's tick 0 is the 528th poll: the gate passes and the
        // first iteration runs, but the page it displays is the cleared one.
        assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        assert!(part.started);
        assert_eq!(part.start_page, PAGE_LO);
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
        // The wait tick shows the same page; the second iteration selects the
        // page the first copied into.
        assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        assert_eq!(part.start_page, PAGE_LO);
        assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        assert_eq!(part.start_page, PAGE_HI);
        // The picture is still below the screen: `startrise` starts at 160 and
        // falls one a vblank, so the copy's top row 70 + startrise is off the
        // bottom for the first ~90 vblanks.
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
        for _ in 3..80 {
            rig.tick(&mut part, false);
        }
        assert!(
            rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
                .iter()
                .any(|&pixel| pixel != 0),
            "the terrain is entering from below at tick 79"
        );
    }

    #[test]
    fn the_part_ends_on_the_music_break_not_the_dead_frame_cap() {
        use sr_engine::music::Music;
        let mut rig = Rig::new();
        let mut part = Coman::at("../../");
        rig.init(&mut part, true).unwrap();
        let mut tick = 0u32;
        loop {
            assert!(tick < FRAME_CAP, "the part outlived the dead frame cap");
            if rig.tick(&mut part, false) == PartStatus::Finished {
                break;
            }
            tick += 1;
        }
        let musplus = rig.music.musplus();
        let position = rig.music.position();
        assert_eq!(
            (position.order, position.row),
            (96, 57),
            "the first published row inside the break's window"
        );
        assert_eq!(musplus, -7, "`row - 64` at MUSIC1 order 96 row 57");
        // The published row enters the break's window at order 96 row 57
        // (~boot 17060 with this part's origin), so the last iteration is the
        // 1,007th — tick 2,014. The terrain itself is already off the bottom
        // by 17,000: `startrise` rises one a vblank through order 96's
        // `zplus == 1` tail.
        assert_eq!(tick, 2_014, "the measured break tick");
    }

    #[test]
    fn the_exit_flag_ends_the_part_where_the_source_checks_it() {
        let mut rig = Rig::new();
        let mut part = Coman::at("../../");
        rig.init(&mut part, true).unwrap();
        assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        assert_eq!(rig.tick(&mut part, true), PartStatus::Finished);
        assert_eq!(rig.tick(&mut part, false), PartStatus::Finished, "sticky");
    }

    #[test]
    fn the_terrain_rises_off_the_bottom_before_the_break() {
        // Order 96 is `zplus == 1`, so from published row 35 the inner body
        // raises `startrise` a vblank; by tick 1,960 the 140-row copy starts
        // below row 200 and the screen is one colour (the reference's 17,000
        // is flat black, `reference-indices.md` §16). The part keeps running
        // off-screen until the break at tick 2,014.
        let mut rig = Rig::new();
        let mut part = Coman::at("../../");
        rig.init(&mut part, true).unwrap();
        for tick in 0..=1_964u32 {
            rig.tick(&mut part, false);
            let pixels = &rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize];
            if tick == 1_850 {
                assert!(pixels.iter().any(|&pixel| pixel != 0), "terrain at 1,850");
            }
            if tick == 1_960 {
                assert!(
                    pixels.iter().all(|&pixel| pixel == 0),
                    "flat black at 1,960: startrise has taken the picture off the screen"
                );
            }
        }
        // The copy's first row is 70 + startrise, so the screen is black once
        // startrise reaches 130; it is still climbing to 160 when the break
        // ends the part.
        assert!(part.startrise >= 130, "startrise {}", part.startrise);
    }

    #[test]
    fn the_wave_tables_are_the_linked_sixty_four_kilobyte_ints() {
        let (wave1, wave2) = read_waves();
        assert_eq!(wave1.len(), WAVE_BYTES);
        assert_eq!(wave2.len(), WAVE_BYTES);
        let range = |bytes: &[u8]| {
            let mut lo = i16::MAX;
            let mut hi = i16::MIN;
            for at in (0..bytes.len()).step_by(2) {
                let word = i16::from_le_bytes([bytes[at], bytes[at + 1]]);
                lo = lo.min(word);
                hi = hi.max(word);
            }
            (lo, hi)
        };
        assert_eq!(range(&wave1), (-33, 41), "W1DTA.BIN's signed words");
        assert_eq!(range(&wave2), (-56, 63), "W2DTA.BIN's signed words");
    }
}
