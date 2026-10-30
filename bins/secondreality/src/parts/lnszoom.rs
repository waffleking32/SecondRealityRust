//! LNS&ZOOM — the face wipe, the lens, and the rotazoomer.
//!
//! `LENS/MAIN.C` (394 lines) over `LENS/ASM.ASM` (566 lines), one binary
//! holding three routines run back to back (`MAIN.C:379-386`):
//!
//! 1. **`part1`** (`:89-131`) — the demon face wipes in with two moving edges,
//!    for at most 300 frames, after a music wait.
//! 2. **`part2`** (`:133-183`) — 715 frames of `drawlens`, a magnifying bubble
//!    steered along `LENS.EXP`'s first path, then a 5-frame hold.
//! 3. **`part3`** (`:185-276`) — the rotazoomer: the face sampled through a
//!    16.16 walker, rotated/zoomed, for 2000 frames, fading out from frame
//!    1872 and ending on a flat white palette.
//!
//! # The screen, and the two layouts in one binary
//!
//! Parts 1 and 2 are mode 13h: 320×200, one byte a pixel, row stride 320.
//! Part 3 calls `inittwk` (`ASM.ASM:185-238`), which clears the DAC, takes
//! CRTC 9 to 0 (a 400-line display), puts the address counter in byte mode
//! (CRTC `0x14 = 0`; CRTC `0x17`'s data byte `0xE3`, written as the `out dx,ax`
//! word `0xe317`), sets the sequencer's memory mode (index 4, data 6) and map
//! mask (index 2, data `0x0F`), clears 64 KB with `rep stosw`, and then sets
//! **maximum scan line 3** — four scanlines a character row. With mode 13h's
//! 400-line vertical total that leaves 100 character rows, each drawn twice in
//! the emulator's 320×200 frame (DOSBox's `is_double_scanned_mode` halves the
//! height and halves `address_line_total`, `vga_draw.cpp`), and the frame the
//! reference files is those 100 rows each repeated twice.
//!
//! The rotazoomer's writes are **Mode X 256-colour planar**: the sequencer's
//! chain-4 is off, so one CPU address is four plane bytes (DOSBox's
//! `VGA_UnchainedVGA_Handler` stores them at `4*addr + plane`), and one
//! displayed byte address is four pixels, one a plane. `rotate` writes each
//! address twice — mask `0x03` with planes 0 and 1, then mask `0x0C` with
//! planes 2 and 3 — so a group of four pixels is `(a, a, b, b)`: the first two
//! from one sample and the last two from another. The reference's frames show
//! exactly that two-pixel repetition.
//!
//! # `rotate` (`ASM.ASM:274-417`), unit for unit
//!
//! The path gives `(x, y, xa, ya)`. `xpos`/`ypos` are `x`/`y` in 16.16;
//! `xadd`/`yadd` are `xa`/`ya << 6`. If `|xa| > |ya|` the walker takes the
//! **rotated** picture (`rotpic90 = back`, built in place from the heap's
//! `rotpic`) and maps `(xa, ya) -> (-ya, xa)` with `xpos/ypos` swapped and
//! negated; the two branches sample the same face along the same vector. The
//! displacement table is accumulated once — 160 steps of the walker, each step
//! `x += yadd`, `y += -xadd`, the accumulated high bytes patched into the
//! unrolled draw code. **Then** the aspect ratio (`* 307 >> 8`) is applied to
//! `xadd`/`yadd` — and to nothing else: the in-row steps keep the uncorrected
//! vector and only the row-to-row base advance is scaled. Applying the aspect
//! before the table (the obvious reading) moves the animation a frame's worth
//! and misses the reference's 4200 entirely; the source's order is what the
//! frame pins.
//!
//! The row loop (`ZOOMYW` = 100) adds the corrected vector to `ypos`/`xpos`,
//! takes `(y & 0xff, x & 0xff)` from their integer parts, and samples
//! `src[(base + disp[n]) & 0xffff]` for `n = 1..=160`. Pixels `2n, 2n+1` of
//! the row take sample `n + 1`, so the row's 320 pixels are 160 samples each
//! shown twice. The port composes the doubled 100→200-row screen the emulator
//! renders.
//!
//! # The lens (`drawlens`, `MAIN.C:49-76`, over `dorow`/`dorow2`/`dorow3`)
//!
//! `LENS.EX1..4` are 116 row entries `{u16 data_offset, u16 count}` followed by
//! a data area at 480 (`CALC.C:153` writes `rowbeg*2 + ymax*4`; `ymax` is 120,
//! so 120*4 = 480). `dorow` (EX1, OR `0x40`) writes `count` pixels from
//! `back[base + offset[i]]` to `video[base + i]` with `base = u + delta`, and
//! returns without drawing below count 4; `dorow2` (EX2/EX3, OR `0x80`/`0xC0`)
//! walks `{dest_offset, source_offset}` pairs; `dorow3` (EX4, no OR) copies
//! `count` bytes at the same offset in both. Every address is 16-bit: the
//! original's `es:[bx+di]`/`ds:[bx+di]` wrap at 64 KB, and the port wraps with
//! `wrapping_add`.
//!
//! The source's `base` is the row's address `u` plus the row's delta; the
//! destination runs `base + i` while the source runs `base + offset[i]` —
//! the tables are a magnified, distorted sampling of the picture around the
//! lens, not a copy.
//!
//! # The palette, `fade` and `fade2` (`MAIN.C:300-357`)
//!
//! `LENS.EXB+16` is the base 64-entry palette. The three overlay banks at
//! 64/128/192 add `LENS.EX0`'s three RGB triples (`(0,5,15)`, `(0,7,26)`,
//! `(0,9,37)`), clamped at 63; the lens's OR bits select them. `fade` is 64
//! blocks of the base ramped to white (`(pal*(63-x) + x*63)/63`) followed by
//! 16 blocks of `pal + (15-x)*5`; the rotazoomer's first 16 frames upload the
//! latter and its last 128 the former. `fade2` is 32 blocks of entries 64..255
//! ramped towards the base (`pal[y] - pal[y%192]*(31-x)/31`); the lens uploads
//! them for `uframe < 96`.
//!
//! # The bugs, preserved
//!
//! - **`if(dis_musplus>-30)`** (`MAIN.C:104`) is missing its `()`: it tests the
//!   function pointer, which is non-zero, so the wait always runs. The port
//!   waits unconditionally ([`PART1_GATE`]).
//! - **The `setpalarea` at `:200` is erased by `inittwk`** (`:201`), whose DAC
//!   clear runs immediately after. The port uploads it and then sets black.
//! - **The 1,536-byte overrun** (`:302`): `memcpy(back+64000, back+64000-1536,
//!   1536)` copies the picture's last 1,536 bytes into the buffer past its end
//!   (the `FILLER` pad). The lens's 16-bit source addresses reach it.
//! - **`rotpic90=back`** (`:191`): the original writes the rotated texture over
//!   the picture buffer, while `rotpic` is the separate heap copy. The port
//!   keeps the two buffers separate and builds the rotation from `rotpic`;
//!   the aliasing is **unobservable**, because nothing reads `back` after
//!   part 3 begins — the wipe and the lens are its only readers, and both are
//!   done by then.
//! - **The `a>3` clamp** (`:176`): the original advances `frame` by
//!   `min(waitb(), 3)` while `uframe` advances by the whole count. The port
//!   runs one frame a tick, so both advance 1:1 and the clamp is
//!   unreachable.
//! - **`rotatez` is dead** (`ASM.ASM:419-564`): identical to `rotate` except
//!   that it adds `256` to `xadd` and subtracts it from `yadd` each row; it is
//!   never called.
//! - **The Scroll Lock border test** (`MAIN.C:36-47`): `waitb` paints the
//!   border when `shiftstatus` bit 4 is set. There is no video border in the
//!   engine's composited frame, and the reference was captured headless; the
//!   test is not modelled.
//!
//! No copper callbacks: the source registers none.

use std::path::Path;

use sr_assets::{load_path, Image};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "lnszoom";

// ------------------------------------------------------------- the screen --

/// `MAIN.C:292` `setvmode(0x13)`: 320 pixels a row, 200 rows, one byte a pixel.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// Mode 13h's row stride, which is also the display's.
const LINE_LEN: u16 = 320;
/// The displayed picture: 64,000 bytes.
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
/// The 16-bit address space the lens tables wrap in.
const VIDEO_MEMORY: usize = 65_536;
/// The rotazoomer's 100 character rows, each two scanlines in the capture.
const ROT_ROWS: usize = 100;
/// Four pixels a Mode X byte address.
const PLANE_BYTES: usize = 4;

// ------------------------------------------------------------- the assets --

/// `MAIN.C:303-304` `lenswid`/`lenshig`: `LENS.EX0`'s first two words.
const LENS_WIDTH: i32 = 152;
const LENS_HEIGHT: i32 = 116;
/// `MAIN.C:306-307` `lensxs`/`lensys`.
const LENS_HALF_WIDTH: i32 = 76;
const LENS_HALF_HEIGHT: i32 = 58;
/// The tables' row count: `CALC.C:75` `for(y=0;y<ymax;y++)`, `ymax` 120 — but
/// only the 116 the C's `lenshig` draws (`ye = lenshig-1 = 115`).
const LENS_ROWS: usize = 116;
/// `CALC.C:153` `rowbeg*2 + ymax*4`: the data area starts at 480.
const LENS_DATA_BASE: usize = 120 * 4;
/// One row entry: `{u16 data_offset, u16 count}`.
const ROW_ENTRY: usize = 4;

const EX0_PATH: &str = "assets/LENS/LENS.EX0";
const EX1_PATH: &str = "assets/LENS/LENS.EX1";
const EX2_PATH: &str = "assets/LENS/LENS.EX2";
const EX3_PATH: &str = "assets/LENS/LENS.EX3";
const EX4_PATH: &str = "assets/LENS/LENS.EX4";
const EXB_PATH: &str = "assets/LENS/LENS.EXB";
const EXP_PATH: &str = "assets/LENS/LENS.EXP";

/// The picture's size inside `LENS.EXB`: header 16 + palette 768. The
/// `load_path` decoder hands the pixels back on their own; the tests use the
/// offset to check them against the raw file.
#[cfg(test)]
const EXB_PICTURE_AT: usize = 784;
/// The 1,536 bytes `MAIN.C:302` copies past the picture's end.
const OVERRUN_BYTES: usize = 1536;

/// `LENS.EXP`'s header: `MAIN.C:296-298`.
const PATH_HEADER: usize = 4;
/// `MAIN.C:297` `pathdata1`: 715 frames × 2 ints.
const PATH1_FRAMES: usize = 715;
/// `MAIN.C:298` `pathdata2`: 2000 frames × 4 ints.
const PATH2_FRAMES: usize = 2000;

// ------------------------------------------------------------ the palette --

/// `MAIN.C:300` `memcpy(palette, lensexb+16, 768)`.
const PALETTE_BYTES: usize = 768;
/// The three overlay banks `MAIN.C:308-325` reads from `LENS.EX0`+4. The
/// values are the file's; the constant is the expectation `init` validates
/// the bytes against.
const EXPECTED_OVERLAYS: [(u8, u8, u8); 3] = [(0, 5, 15), (0, 7, 26), (0, 9, 37)];
/// `MAIN.C:331-347`: 64 ramp blocks and 16 white-step blocks.
const FADE_BLOCKS: usize = 64 + 16;
/// One fade block: 64 entries × 3.
const FADE_BLOCK: usize = 64 * 3;
/// `MAIN.C:349-357`: 32 blocks of entries 64..255.
const FADE2_BLOCKS: usize = 32;
/// One fade2 block: 192 entries × 3.
const FADE2_BLOCK: usize = 192 * 3;

// ---------------------------------------------------------------- the acts --

/// `MAIN.C:104` `while(!dis_exit() && dis_musplus()<-6)`: the first gate.
const PART1_GATE: i32 = -6;
/// `MAIN.C:107` `frame<300`: part 1's length in frames.
const PART1_FRAMES: u32 = 300;
/// `MAIN.C:109` `frame<80`: the wipe's frames.
const WIPE_FRAMES: u32 = 80;
/// `MAIN.C:112` `for(c=0;c<6;c++)`: six copies a frame.
const WIPE_COPIES: usize = 6;
/// `MAIN.C:144` `uframe<715`: part 2's length.
const LENS_FRAMES: u32 = 715;
/// `MAIN.C:146` `uframe<96`: the frames whose `fade2` block is uploaded.
const LENS_FADE_FRAMES: u32 = 96;
/// `MAIN.C:179` `uframe<720`: the 5-frame hold.
const LENS_HOLD_END: u32 = 720;
/// `MAIN.C:380` `while(!dis_exit() && dis_musplus()<-20)`: the inter-act gate.
const LENS_GATE: i32 = -20;
/// `MAIN.C:211` `frame<2000`: part 3's length.
const ROT_FRAMES: u32 = 2000;
/// `MAIN.C:213` `if(dis_musplus()>-4) break;`.
const ROT_BREAK: i32 = -4;
/// `MAIN.C:261` `if(frame>2000-128)`: the fade's first frame.
const ROT_FADE_START: u32 = ROT_FRAMES - 128;
/// `MAIN.C:268` `if(frame<16)`: the white-step uploads.
const ROT_WHITE_STEPS: u32 = 16;
/// `MAIN.C:262-265`: `a=(frame-1872)/2`, clamped at 63.
const ROT_FADE_CLAMP: i32 = 63;

/// `MAIN.C:213`'s counterpart in the tree: the part3 setup's cost. The source's
/// explicit waits between the last lens draw and the first rotazoomer frame
/// are seven (the loop's exit, five hold frames and `MAIN.C:199`'s `waitb`);
/// the reference's pictures need eight — its 4200 is path frame 552 and its
/// 4900 path frame 1252 (`bins/secondreality/tests/lnszoom.rs`), both one
/// frame after `T3 = 3648`, while the lens's last draw is at 3640. The
/// rotpic90 rotation and `inittwk`'s 64 KB clear cost the release a vblank, so
/// the port charges the setup two frames (`RotSetup` stages 0 and 1).
const ROT_SETUP_FRAMES: u32 = 2;

/// How many polls [`Lnszoom::init`] makes to reach the reference's position at
/// the part's tick 0: `reference/music/second3/trace.csv` holds MUSIC1 order
/// 45 row 59 at boot 2452 (the frame the part's `_dis_setmframe(0)` is read
/// on), and the player publishes 45:59 on its 491st poll after a seek to order
/// 45 — so 490 polls precede the engine's tick-0 poll. Measured; the same
/// pre-roll puts the 48:44 row at tick 473 (boot 2925), the frame the trace
/// holds it, which is what puts the lens at the reference's frame 0 at tick
/// 474. See `docs/fidelity/lnszoom.md` §4.
const MUSIC_PRE_ROLL: u32 = 490;

/// The capture frame the port's tick 0 stands at — **a mapping, not an
/// anchor.** The part's start is the window (2452, 2456]
/// (`reference-indices.md` §12.5); `shell::boot_anchor("lnszoom")` stays
/// `None`, a run of this part numbers its own frames from zero, and its
/// manifest says `index_origin: "part"`, `anchor: null`. The tests compare a
/// reference index at `index - REFERENCE_FIRST_INDEX`; the mapping is pinned
/// by the trace's 45:59 at 2452, the lens's 3400 = frame 474, the
/// rotazoomer's 4200 = frame 552 and 4900 = frame 1252, and the white at
/// 5650.
#[cfg(test)]
const REFERENCE_FIRST_INDEX: u32 = 2452;

// --------------------------------------------------------------- the part --

/// Where in `MAIN.C`'s flow the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `:104`: the `musplus < -6` wait (with the missing `()`).
    Gate,
    /// `:107-130`: the wipe; `frame` is the source's counter.
    Wipe { frame: u32 },
    /// `:380`: the `musplus < -20` wait between part 1 and part 2.
    LensWait,
    /// `:144-178`: the lens; `uframe` is the loop's counter, `frame` the
    /// path's (they would diverge only through the `a>3` clamp, unreachable
    /// at one frame a tick).
    Lens { uframe: u32, frame: u32 },
    /// `:179-182`: the five-frame hold.
    LensHold { uframe: u32 },
    /// `:191-201`: the rotpic90 rotation then `setpalarea` + `inittwk`.
    RotSetup { stage: u32 },
    /// `:211-272`: the rotazoomer.
    Rot { frame: u32 },
    /// The part has ended and keeps its last frame.
    Done,
}

/// LNS&ZOOM: the face, the lens tables and the rotazoomer.
pub struct Lnszoom {
    /// The asset root: empty for a run (the repository root), `../../` for a
    /// unit test (the crate's own directory).
    root: &'static str,
    /// `LENS.EX0`: dimensions and the three overlay triples.
    ex0: Vec<u8>,
    /// `LENS.EX1..4`, in the source's order.
    tables: [Vec<u8>; 4],
    /// `LENS.EXP`'s pathdata1/pathdata2, as signed 16-bit ints.
    path1: Vec<i16>,
    path2: Vec<i16>,
    /// `back`: the 64,000-byte picture plus `MAIN.C:302`'s 1,536-byte overrun.
    back: Vec<u8>,
    /// The generated 256-entry palette, six bits an entry.
    palette: Vec<u8>,
    /// `fade`: 80 blocks of 192 bytes.
    fade: Vec<u8>,
    /// `fade2`: 32 blocks of 576 bytes.
    fade2: Vec<u8>,
    /// `rotpic`: the 256×256 face the rotazoomer samples.
    rotpic: Vec<u8>,
    /// `rotpic90`: `back` overwritten with the rotated `rotpic` in part 3.
    rotpic90: Vec<u8>,
    /// Video memory: mode 13h's linear bytes in parts 1–2, Mode X's four
    /// plane bytes an address in part 3.
    video: Vec<u8>,
    /// `firfade1`, `firfade2` and their per-copy steps (`MAIN.C:84-103`).
    firfade1: Vec<i32>,
    firfade2: Vec<i32>,
    firfade1a: Vec<i32>,
    firfade2a: Vec<i32>,
    phase: Phase,
    /// The part's tick counter, incremented at the top of `frame`: the call
    /// that draws dump index `n` leaves this at `n + 1`.
    tick: u32,
}

impl Lnszoom {
    /// A part with nothing loaded: `init` reads the assets.
    pub fn new() -> Lnszoom {
        Lnszoom {
            root: "",
            ex0: Vec::new(),
            tables: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            path1: Vec::new(),
            path2: Vec::new(),
            back: Vec::new(),
            palette: Vec::new(),
            fade: Vec::new(),
            fade2: Vec::new(),
            rotpic: Vec::new(),
            rotpic90: Vec::new(),
            video: Vec::new(),
            firfade1: Vec::new(),
            firfade2: Vec::new(),
            firfade1a: Vec::new(),
            firfade2a: Vec::new(),
            phase: Phase::Gate,
            tick: 0,
        }
    }

    /// [`new`](Lnszoom::new) reading its assets under `root`.
    #[cfg(test)]
    fn at(root: &'static str) -> Lnszoom {
        Lnszoom {
            root,
            ..Lnszoom::new()
        }
    }

    fn asset(&self, path: &str) -> String {
        format!("{}{}", self.root, path)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, PartError> {
        let full = self.asset(path);
        std::fs::read(&full).map_err(|error| PartError {
            part: NAME,
            message: format!("{full}: {error}"),
        })
    }

    fn read_image(&self, path: &str) -> Result<Image, PartError> {
        let full = self.asset(path);
        load_path(Path::new(&full)).map_err(|error| PartError {
            part: NAME,
            message: format!("{full}: {error}"),
        })
    }

    /// `MAIN.C:290-292`: mode 13h's display state.
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

    /// `LENS.EX0`+4's nine bytes: the three overlay RGB triples.
    fn ex0_overlays(&self) -> [(u8, u8, u8); 3] {
        [
            (self.ex0[4], self.ex0[5], self.ex0[6]),
            (self.ex0[7], self.ex0[8], self.ex0[9]),
            (self.ex0[10], self.ex0[11], self.ex0[12]),
        ]
    }

    /// `MAIN.C:300-325`: the base palette and the three overlay banks.
    fn build_palette(&self, base: &Palette, overlays: [(u8, u8, u8); 3]) -> Vec<u8> {
        let mut palette = vec![0u8; PALETTE_BYTES];
        for index in 0..256 {
            let rgb = base.entry(index as u8);
            palette[index * 3..index * 3 + 3].copy_from_slice(&rgb);
        }
        for (i, (r, g, b)) in overlays.iter().enumerate() {
            let bank = (i + 1) * 64 * 3;
            for at in (0..64 * 3).step_by(3) {
                for (channel, delta) in [*r, *g, *b].iter().enumerate() {
                    let value = palette[at + channel] + delta;
                    palette[bank + at + channel] = if value > 63 { 63 } else { value };
                }
            }
        }
        palette
    }

    /// `MAIN.C:330-357`: the two fade banks, integer arithmetic as the C
    /// writes it (i32, truncating division).
    fn build_fades(&self) -> (Vec<u8>, Vec<u8>) {
        let palette = &self.palette;
        let pal = |at: usize| i32::from(palette[at]);
        let mut fade = Vec::with_capacity(FADE_BLOCKS * FADE_BLOCK);
        for x in 0..64i32 {
            for y in 0..FADE_BLOCK {
                let a = (pal(y) * (63 - x) + x * 63) / 63;
                fade.push(a as u8);
            }
        }
        for x in 0..16i32 {
            for y in 0..FADE_BLOCK {
                let a = pal(y) + (15 - x) * 5;
                fade.push(if a > 63 { 63 } else { a } as u8);
            }
        }
        let mut fade2 = Vec::with_capacity(FADE2_BLOCKS * FADE2_BLOCK);
        for x in 0..32i32 {
            for y in 64 * 3..256 * 3 {
                let base = y % (64 * 3);
                let a = pal(y) - pal(base) * (31 - x) / 31;
                fade2.push(a as u8);
            }
        }
        (fade, fade2)
    }

    /// `MAIN.C:358-368`: `rotpic[x + y*256] = back[x + 32 + a*320]` with
    /// `a = y*10/11 - 36/2`, clamped to row 0 outside 0..199.
    fn build_rotpic(back: &[u8]) -> Vec<u8> {
        let mut rotpic = vec![0u8; VIDEO_MEMORY];
        for x in 0..256usize {
            for y in 0..256usize {
                let mut a = y as i32 * 10 / 11 - 36 / 2;
                if !(0..=199).contains(&a) {
                    a = 0;
                }
                rotpic[x + y * 256] = back[x + 32 + a as usize * 320];
            }
        }
        rotpic
    }

    /// `part3`'s `rotpic90=back` and the in-place 90° rotation
    /// (`MAIN.C:191-198`): the source reads the separate `rotpic`.
    fn build_rotpic90(rotpic: &[u8]) -> Vec<u8> {
        let mut rotated = vec![0u8; VIDEO_MEMORY];
        for x in 0..256usize {
            for y in 0..256usize {
                rotated[x + y * 256] = rotpic[y + (255 - x) * 256];
            }
        }
        rotated
    }

    /// `MAIN.C:96-103`: the wipe's position and step tables.
    fn build_firfade() -> (Vec<i32>, Vec<i32>, Vec<i32>, Vec<i32>) {
        let mut f1 = Vec::with_capacity(200);
        let mut f2 = Vec::with_capacity(200);
        let mut a1 = Vec::with_capacity(200);
        let mut a2 = Vec::with_capacity(200);
        for b in 0..200i32 {
            a1.push((19 + b / 5 + 4) & !7);
            a2.push((-(19 + (199 - b) / 5 + 4)) & !7);
            f1.push(170 * 64 + (100 - b) * 50);
            f2.push(170 * 64 + (100 - b) * 50);
        }
        (f1, f2, a1, a2)
    }

    /// One `setpalarea(data, start, count)`.
    fn upload(&self, ctx: &mut PartCtx, start: usize, count: usize, data: &[u8]) {
        for index in 0..count {
            let at = index * 3;
            ctx.set_palette_entry(
                (start + index) as u8,
                [data[at], data[at + 1], data[at + 2]],
            );
        }
    }

    /// The generated 256-entry palette, as the engine's type.
    fn live_palette(&self) -> Palette {
        let mut palette = Palette::new();
        for index in 0..256 {
            let at = index * 3;
            palette.set_entry(
                index as u8,
                [self.palette[at], self.palette[at + 1], self.palette[at + 2]],
            );
        }
        palette
    }

    /// The white the part exits on (`MAIN.C:274-275`).
    fn set_white(&self, ctx: &mut PartCtx) {
        let mut white = Palette::new();
        for index in 0..256u16 {
            white.set_entry(index as u8, [63, 63, 63]);
        }
        ctx.set_palette(white);
    }

    /// `MAIN.C:113-126`: six copies of the two-edge pixel walk. Each copy
    /// writes one picture byte per row at each edge, at the same linear index
    /// in `back` and `video`; writes outside the displayed 64,000 bytes are
    /// invisible and dropped.
    fn wipe(&mut self) {
        for _ in 0..WIPE_COPIES {
            for y in 0..SCREEN_HEIGHT as usize {
                for (edge, step) in [(&mut self.firfade1, &self.firfade1a),
                                     (&mut self.firfade2, &self.firfade2a)] {
                    let index = y as i32 * LINE_LEN as i32 + (edge[y] >> 6);
                    if index >= 0 && (index as usize) < VIDEO_BYTES {
                        self.video[index as usize] = self.back[index as usize];
                    }
                    edge[y] += step[y];
                }
            }
        }
    }

    /// `MAIN.C:49-76` `drawlens`: the top half's rows at `u1` and the bottom
    /// half's reversed rows at `u2`, four table passes each.
    fn draw_lens(&mut self, x0: i32, y0: i32) {
        let mut u1 = (x0 - LENS_HALF_WIDTH) + (y0 - LENS_HALF_HEIGHT) * 320;
        let mut u2 = (x0 - LENS_HALF_WIDTH) + (y0 + LENS_HALF_HEIGHT - 1) * 320;
        for y in 0..LENS_HALF_HEIGHT as usize {
            if (0..=VIDEO_BYTES as i32).contains(&u1) {
                let u = u1 as u16;
                self.dorow(0, u, y, 0x40);
                self.dorow2(1, u, y, 0x80);
                self.dorow2(2, u, y, 0xC0);
                self.dorow3(u, y);
            }
            u1 += 320;
            let row = LENS_ROWS - 1 - y;
            if (0..=VIDEO_BYTES as i32).contains(&u2) {
                let u = u2 as u16;
                self.dorow(0, u, row, 0x40);
                self.dorow2(1, u, row, 0x80);
                self.dorow2(2, u, row, 0xC0);
                self.dorow3(u, row);
            }
            u2 -= 320;
        }
    }

    /// `{u16 data_offset, u16 count}` for row `y` of table `which`.
    fn lens_row(&self, which: usize, y: usize) -> (usize, usize) {
        let table = &self.tables[which];
        let at = y * ROW_ENTRY;
        (
            usize::from(u16::from_le_bytes([table[at], table[at + 1]])),
            usize::from(u16::from_le_bytes([table[at + 2], table[at + 3]])),
        )
    }

    fn row_delta(&self, which: usize, at: usize) -> u16 {
        let t = &self.tables[which];
        i16::from_le_bytes([t[at], t[at + 1]]) as u16
    }

    fn lens_word(&self, which: usize, at: usize) -> u16 {
        let t = &self.tables[which];
        u16::from_le_bytes([t[at], t[at + 1]])
    }

    /// `dorow` (`ASM.ASM:22-90`): `count` pixels, `back[base+offset[i]]` to
    /// `video[base+i]`, OR `or`. Counts below four draw nothing.
    fn dorow(&mut self, which: usize, u: u16, y: usize, or: u8) {
        let (at, count) = self.lens_row(which, y);
        if count < 4 {
            return;
        }
        let base = u.wrapping_add(self.row_delta(which, at));
        for i in 0..count {
            let offset = self.lens_word(which, at + 2 + 2 * i);
            let source = base.wrapping_add(offset) as usize;
            self.video[base.wrapping_add(i as u16) as usize] = self.back[source] | or;
        }
    }

    /// `dorow2` (`ASM.ASM:92-132`): `{dest, source}` offset pairs, OR `or`.
    fn dorow2(&mut self, which: usize, u: u16, y: usize, or: u8) {
        let (at, count) = self.lens_row(which, y);
        if count == 0 {
            return;
        }
        let base = u.wrapping_add(self.row_delta(which, at));
        for i in 0..count {
            let dest = self.lens_word(which, at + 2 + 4 * i);
            let source = self.lens_word(which, at + 4 + 4 * i);
            self.video[base.wrapping_add(dest) as usize] =
                self.back[base.wrapping_add(source) as usize] | or;
        }
    }

    /// `dorow3` (`ASM.ASM:134-168`): `count` bytes copied at one offset.
    fn dorow3(&mut self, u: u16, y: usize) {
        let (at, count) = self.lens_row(3, y);
        if count == 0 {
            return;
        }
        let base = u.wrapping_add(self.row_delta(3, at));
        for i in 0..count {
            let offset = base.wrapping_add(self.lens_word(3, at + 2 + 2 * i));
            self.video[offset as usize] = self.back[offset as usize];
        }
    }

    /// The 160-step displacement walker `ASM.ASM:335-358` accumulates, before
    /// the aspect ratio is applied.
    fn displacements(xadd: i32, yadd: i32) -> [u16; 160] {
        let mut disp = [0u16; 160];
        let (mut yv, mut al) = (0u16, 0u8);
        let (mut xv, mut ah) = (0u16, 0u8);
        let ylo = yadd as u16;
        let yhi = (yadd as u32 >> 16) as u8;
        let nx = xadd.wrapping_neg();
        let xlo = nx as u16;
        let xhi = (nx as u32 >> 16) as u8;
        for entry in disp.iter_mut() {
            let (ny, cy) = yv.overflowing_add(ylo);
            yv = ny;
            al = al.wrapping_add(yhi).wrapping_add(u8::from(cy));
            let (nxv, cx) = xv.overflowing_add(xlo);
            xv = nxv;
            ah = ah.wrapping_add(xhi).wrapping_add(u8::from(cx));
            *entry = u16::from(ah) << 8 | u16::from(al);
        }
        disp
    }

    /// `rotate` (`ASM.ASM:274-417`) over the path's four words, composing the
    /// emulator's doubled 100→200-row picture.
    fn rotate(&self, ctx: &mut PartCtx, frame: usize) {
        let x = i32::from(self.path2[frame * 4]);
        let y = i32::from(self.path2[frame * 4 + 1]);
        let xa = i32::from(self.path2[frame * 4 + 2]);
        let ya = i32::from(self.path2[frame * 4 + 3]);

        let mut xpos = x.wrapping_shl(16);
        let mut ypos = y.wrapping_shl(16);
        let mut xadd = xa.wrapping_shl(6);
        let mut yadd = ya.wrapping_shl(6);

        // The direction test and the axis swap (`@@s1`–`@@s3`).
        let use_rotated = xadd.unsigned_abs() > yadd.unsigned_abs();
        if use_rotated {
            std::mem::swap(&mut xadd, &mut yadd);
            xadd = xadd.wrapping_neg();
            let (old_x, old_y) = (xpos, ypos);
            xpos = old_y.wrapping_neg();
            ypos = old_x;
        }
        let source: &[u8] = if use_rotated {
            &self.rotpic90
        } else {
            &self.rotpic
        };

        // The in-row steps keep the uncorrected vector; the aspect ratio
        // scales only the row advance (`;aspect ratio` follows the patched
        // table in `ASM.ASM`).
        let disp = Self::displacements(xadd, yadd);
        let aspect = |v: i32| (307u32.wrapping_mul(v as u32) as i32) >> 8;
        let xrow = aspect(xadd);
        let yrow = aspect(yadd);

        let surface = ctx.surface.pixels_mut();
        for row in 0..ROT_ROWS {
            ypos = ypos.wrapping_add(yrow);
            xpos = xpos.wrapping_add(xrow);
            let base = (((ypos as u32 >> 16) & 0xff) << 8 | ((xpos as u32 >> 16) & 0xff)) as usize;
            let (out, next) = surface.split_at_mut((2 * row + 1) * LINE_LEN as usize);
            let out = &mut out[2 * row * LINE_LEN as usize..][..LINE_LEN as usize];
            let next = &mut next[..LINE_LEN as usize];
            for (n, step) in disp.iter().enumerate() {
                let value = source[(base + usize::from(*step)) & 0xffff];
                out[2 * n] = value;
                out[2 * n + 1] = value;
                next[2 * n] = value;
                next[2 * n + 1] = value;
            }
        }
    }

    /// Parts 1–2's display: the first 64,000 bytes of video memory, linear.
    fn compose_linear(&self, ctx: &mut PartCtx) {
        ctx.surface.pixels_mut()[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
    }

    /// Part 3's display: 100 character rows of four pixels an address, each
    /// row shown twice (the emulator's `address_line_total` is 2).
    fn compose_planar(&self, ctx: &mut PartCtx, rows: usize) {
        let surface = ctx.surface.pixels_mut();
        for row in 0..rows {
            let line = 2 * row * LINE_LEN as usize;
            for address in 0..SCREEN_WIDTH as usize / PLANE_BYTES {
                let at = PLANE_BYTES * (row * (SCREEN_WIDTH as usize / PLANE_BYTES) + address);
                for plane in 0..PLANE_BYTES {
                    let value = self.video[at + plane];
                    surface[line + 4 * address + plane] = value;
                    surface[line + LINE_LEN as usize + 4 * address + plane] = value;
                }
            }
        }
    }
}

impl Part for Lnszoom {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:278-372`: the loads, the generated palettes and tables, and the
    /// music catch-up. The source's `waitb(); setpalarea(palette,0,256)` is
    /// the initial palette; the mode set's clear is the zeroed video memory.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // All three acts wait on the music: `:104`, `:380`, `:213`.
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (LENS/MAIN.C:104, :380, :213) and the run has no music to wait on",
        )?;

        // The reference's player is already at order 45 row 59 on the tick this
        // part's tick 0 stands at; an entry run starts the player at the seek's
        // row 0, so the difference is made here. A part reached mid-chain finds
        // the player already past this stretch, so the catch-up is the entry
        // run's (`music_fresh`) alone.
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
        }

        self.ex0 = self.read(EX0_PATH)?;
        if self.ex0.len() < 13 {
            return Err(PartError {
                part: NAME,
                message: format!("{EX0_PATH} is {} bytes; the part reads 13", self.ex0.len()),
            });
        }
        let width = i32::from(i16::from_le_bytes([self.ex0[0], self.ex0[1]]));
        let height = i32::from(i16::from_le_bytes([self.ex0[2], self.ex0[3]]));
        if (width, height) != (LENS_WIDTH, LENS_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{EX0_PATH} declares a {width}x{height} lens; the source draws \
                     {LENS_WIDTH}x{LENS_HEIGHT}"
                ),
            });
        }
        let overlays = self.ex0_overlays();
        if overlays != EXPECTED_OVERLAYS {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{EX0_PATH} carries the overlay triples {overlays:?}; the source's build \
                     carries {EXPECTED_OVERLAYS:?}"
                ),
            });
        }
        self.tables = [
            self.read(EX1_PATH)?,
            self.read(EX2_PATH)?,
            self.read(EX3_PATH)?,
            self.read(EX4_PATH)?,
        ];
        for (which, path) in [EX1_PATH, EX2_PATH, EX3_PATH, EX4_PATH].iter().enumerate() {
            let table = &self.tables[which];
            if table.len() < LENS_ROWS * ROW_ENTRY {
                return Err(PartError {
                    part: NAME,
                    message: format!(
                        "{path} is {} bytes; {LENS_ROWS} row entries need {}",
                        table.len(),
                        LENS_ROWS * ROW_ENTRY
                    ),
                });
            }
            // Every row the blitter reads must be inside the file. Rows with a
            // zero count (EX2/3/4) or under four (EX1) are never read, which is
            // why those rows may point at the file's end.
            for y in 0..LENS_ROWS {
                let (at, count) = self.lens_row(which, y);
                let read = if which == 0 { count >= 4 } else { count > 0 };
                if read {
                    let bytes = match which {
                        // EX2/EX3 walk `{dest, source}` pairs: four bytes an entry.
                        1 | 2 => 2 + 4 * count,
                        // EX1/EX4 walk one offset word an entry.
                        _ => 2 + 2 * count,
                    };
                    if at < LENS_DATA_BASE || at + bytes > table.len() {
                        return Err(PartError {
                            part: NAME,
                            message: format!(
                                "{path}: row {y}'s data at {at} for {count} entries runs \
                                 outside the file's {} bytes",
                                table.len()
                            ),
                        });
                    }
                }
            }
        }

        // `MAIN.C:300-301`: the palette at +16 and the far picture at +784.
        let image = self.read_image(EXB_PATH)?;
        if (image.width(), image.height()) != (SCREEN_WIDTH, SCREEN_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{EXB_PATH} is {}x{}; the part reads {SCREEN_WIDTH}x{SCREEN_HEIGHT}",
                    image.width(),
                    image.height()
                ),
            });
        }
        let picture = image.pixels();
        if picture.len() < VIDEO_BYTES {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{EXB_PATH} carries {} pixel bytes; the part reads {VIDEO_BYTES}",
                    picture.len()
                ),
            });
        }
        // `back+64000` and the overrun: the copy's source is the last 1,536
        // picture bytes and its destination the 1,536 bytes past them.
        self.back = vec![0u8; VIDEO_MEMORY];
        self.back[..VIDEO_BYTES].copy_from_slice(&picture[..VIDEO_BYTES]);
        let from = VIDEO_BYTES - OVERRUN_BYTES;
        let tail = self.back[from..VIDEO_BYTES].to_vec();
        self.back[VIDEO_BYTES..].copy_from_slice(&tail);

        self.palette = self.build_palette(&image.palette, overlays);
        let (fade, fade2) = self.build_fades();
        self.fade = fade;
        self.fade2 = fade2;
        self.rotpic = Self::build_rotpic(&self.back);

        // `MAIN.C:296-298`: `a = *(int *)(lensexp+2)`, `pathdata1` at +4 and
        // `pathdata2` at `+4 + 2*a` bytes.
        let exp = self.read(EXP_PATH)?;
        if exp.len() < PATH_HEADER {
            return Err(PartError {
                part: NAME,
                message: format!("{EXP_PATH} is {} bytes; its header is 4", exp.len()),
            });
        }
        let count = usize::from(u16::from_le_bytes([exp[2], exp[3]]));
        let path2_at = PATH_HEADER + 2 * count;
        if count != PATH1_FRAMES * 2 || exp.len() < path2_at + PATH2_FRAMES * 4 * 2 {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{EXP_PATH} declares {count} pathdata1 ints and {} bytes; the source reads \
                     {} and {}",
                    exp.len(),
                    PATH1_FRAMES * 2,
                    PATH2_FRAMES * 4 * 2
                ),
            });
        }
        let words = |at: usize, n: usize| -> Vec<i16> {
            (0..n)
                .map(|i| i16::from_le_bytes([exp[at + 2 * i], exp[at + 2 * i + 1]]))
                .collect()
        };
        self.path1 = words(PATH_HEADER, 2 * PATH1_FRAMES);
        self.path2 = words(path2_at, 4 * PATH2_FRAMES);

        let (f1, f2, a1, a2) = Self::build_firfade();
        self.firfade1 = f1;
        self.firfade2 = f2;
        self.firfade1a = a1;
        self.firfade2a = a2;

        self.rotpic90 = Vec::new();
        self.video = vec![0u8; VIDEO_MEMORY];
        self.phase = Phase::Gate;
        self.tick = 0;

        // `:290-292`, the mode set's clear, and `:370-371`'s palette upload.
        Self::set_display(ctx);
        self.compose_linear(ctx);
        let palette = self.live_palette();
        ctx.set_palette(palette);
        Ok(())
    }

    /// One tick of `MAIN.C:89-276`, from the first wait to the white exit.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Gate => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                // The missing `()` in `if(dis_musplus>-30)` makes the guard the
                // function pointer, which is non-zero: the wait always runs.
                let musplus = ctx.musplus();
                if musplus < PART1_GATE {
                    self.compose_linear(ctx);
                    return PartStatus::Continuing;
                }
                // `:105-106` `dis_waitb(); dis_setmframe(0);` — the wait is the
                // tick boundary into the wipe.
                if let Some(music) = ctx.music() {
                    music.setmframe(0);
                }
                self.phase = Phase::Wipe { frame: 0 };
                self.compose_linear(ctx);
                PartStatus::Continuing
            }
            Phase::Wipe { frame } => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                if frame < WIPE_FRAMES {
                    self.wipe();
                }
                self.phase = if frame + 1 == PART1_FRAMES {
                    Phase::LensWait
                } else {
                    Phase::Wipe { frame: frame + 1 }
                };
                self.compose_linear(ctx);
                PartStatus::Continuing
            }
            Phase::LensWait => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                let musplus = ctx.musplus();
                if musplus < LENS_GATE {
                    self.compose_linear(ctx);
                    return PartStatus::Continuing;
                }
                // `:381` `dis_waitb()` is the boundary: the lens draws next tick.
                self.phase = Phase::Lens { uframe: 0, frame: 0 };
                self.compose_linear(ctx);
                PartStatus::Continuing
            }
            Phase::Lens { uframe, frame } => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                // `:146-151`: `a=(uframe-32)/2`, clamped at 0, uploaded to
                // entries 64..255 from that fade2 block.
                if uframe < LENS_FADE_FRAMES {
                    let mut a = (uframe as i32 - 32) / 2;
                    if a < 0 {
                        a = 0;
                    }
                    let block = a as usize * FADE2_BLOCK;
                    self.upload(ctx, 64, 192, &self.fade2[block..block + FADE2_BLOCK]);
                }
                // `:170-172`: the shipped path.
                let x = i32::from(self.path1[frame as usize * 2]);
                let y = i32::from(self.path1[frame as usize * 2 + 1]);
                self.draw_lens(x, y);
                self.compose_linear(ctx);

                // `:174-177` `a=waitb(); uframe+=a; if(a>3)a=3; frame+=a;`.
                let uframe = uframe + 1;
                self.phase = if uframe >= LENS_FRAMES {
                    Phase::LensHold { uframe }
                } else {
                    Phase::Lens {
                        uframe,
                        frame: frame + 1,
                    }
                };
                PartStatus::Continuing
            }
            Phase::LensHold { uframe } => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                let uframe = uframe + 1;
                self.phase = if uframe >= LENS_HOLD_END {
                    Phase::RotSetup { stage: 0 }
                } else {
                    Phase::LensHold { uframe }
                };
                self.compose_linear(ctx);
                PartStatus::Continuing
            }
            Phase::RotSetup { stage } => {
                if ctx.exit_requested {
                    self.set_white(ctx);
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                if stage == 0 {
                    // `:191-198`: the rotation into `back`; the `waitb()` that
                    // follows is the boundary, charged `ROT_SETUP_FRAMES`.
                    if self.rotpic90.is_empty() {
                        self.rotpic90 = Self::build_rotpic90(&self.rotpic);
                    }
                    self.compose_linear(ctx);
                } else {
                    // `:200-201`: the upload is immediately erased by
                    // `inittwk`'s DAC clear; the video memory is cleared and
                    // the rotazoomer's planar layout takes over.
                    let block = 64 * FADE_BLOCK;
                    self.upload(ctx, 0, 64, &self.fade[block..block + FADE_BLOCK]);
                    ctx.set_palette(Palette::new());
                    self.video.fill(0);
                    self.compose_planar(ctx, ROT_ROWS);
                }
                self.phase = if stage + 1 >= ROT_SETUP_FRAMES {
                    Phase::Rot { frame: 0 }
                } else {
                    Phase::RotSetup { stage: stage + 1 }
                };
                PartStatus::Continuing
            }
            Phase::Rot { frame } => {
                // `:211-213`: the exit and the music break both leave the white
                // palette behind (`:274-275`).
                if ctx.exit_requested {
                    self.set_white(ctx);
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                let musplus = ctx.musplus();
                if musplus > ROT_BREAK {
                    self.set_white(ctx);
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                self.rotate(ctx, frame as usize);
                let frame = frame + 1;
                // `:261-267`: the fade-out from frame 1872.
                if frame > ROT_FADE_START {
                    let mut a = (frame - ROT_FADE_START) as i32 / 2;
                    if a > ROT_FADE_CLAMP {
                        a = ROT_FADE_CLAMP;
                    }
                    let block = a as usize * FADE_BLOCK;
                    self.upload(ctx, 0, 64, &self.fade[block..block + FADE_BLOCK]);
                }
                // `:268-271`: the white steps of the first 16 frames.
                if frame < ROT_WHITE_STEPS {
                    let block = (64 + frame) as usize * FADE_BLOCK;
                    self.upload(ctx, 0, 64, &self.fade[block..block + FADE_BLOCK]);
                }
                if frame >= ROT_FRAMES {
                    self.set_white(ctx);
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                self.phase = Phase::Rot { frame };
                PartStatus::Continuing
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{Music, MusicPosition};
    use sr_engine::view::View;
    use sr_engine::{dac_expand, Surface};

    use crate::shell::{SURFACE_HEIGHT, SURFACE_WIDTH};

    /// A player scripted one `(order, row, musplus)` reading a tick.
    struct Player {
        script: Vec<(u8, u8, i32)>,
        at: usize,
        polls: u32,
        mframe: i32,
    }

    impl Player {
        fn scripted(script: Vec<(u8, u8, i32)>) -> Player {
            Player {
                script,
                at: 0,
                polls: 0,
                mframe: 0,
            }
        }

        /// The music never holds a gate: `musplus` is -5, which passes both
        /// waits and never the part 3 break.
        fn running(last_tick: u32) -> Player {
            Player::scripted(vec![(45, 59, -5); last_tick as usize + 1])
        }

        /// A script whose part 3 break fires from `break_tick`.
        fn breaking(break_tick: u32, last_tick: u32) -> Player {
            let mut script = vec![(45, 59, -5); last_tick as usize + 1];
            for entry in script.iter_mut().skip(break_tick as usize) {
                *entry = (60, 60, 0);
            }
            Player::scripted(script)
        }
    }

    impl Music for Player {
        fn poll(&mut self) {
            self.polls += 1;
        }

        fn position(&self) -> MusicPosition {
            let (order, row, _) = self.script.get(self.at).copied().unwrap_or((45, 59, -5));
            MusicPosition {
                order,
                row,
                ..MusicPosition::default()
            }
        }

        fn musplus(&mut self) -> i32 {
            self.script
                .get(self.at)
                .map_or(-32, |(_, _, musplus)| *musplus)
        }

        fn musrow(&mut self) -> i32 {
            i32::from(self.position().row)
        }

        fn muscode(&mut self, wanted: i32) -> i32 {
            sr_engine::music::muscode_from(MusicPosition::default(), wanted)
        }

        fn sync(&mut self) -> i32 {
            unreachable!("`LENS/MAIN.C` never syncs")
        }

        fn getmframe(&mut self) -> i32 {
            self.mframe
        }

        fn setmframe(&mut self, frame: i32) {
            self.mframe = frame;
        }
    }

    trait RigMusic: Music {
        fn seek_tick(&mut self, tick: u32);
        fn pre_tick(&mut self) {}
    }

    impl RigMusic for Player {
        fn seek_tick(&mut self, tick: u32) {
            self.at = tick as usize;
        }
    }

    impl RigMusic for crate::audio::PlayerMusic {
        fn seek_tick(&mut self, _tick: u32) {}
        fn pre_tick(&mut self) {
            self.poll();
        }
    }

    struct Rig<M: RigMusic + 'static> {
        surface: Surface,
        view: View,
        palette: Palette,
        copper: CopperRegistry,
        player: M,
    }

    impl<M: RigMusic + 'static> Rig<M> {
        fn new(player: M) -> Rig<M> {
            Rig {
                surface: Surface::new(SURFACE_WIDTH, SURFACE_HEIGHT),
                view: View::new(SCREEN_WIDTH, SCREEN_HEIGHT),
                palette: Palette::new(),
                copper: CopperRegistry::new(),
                player,
            }
        }

        fn ctx_with(&mut self, frame: u32, exit: bool, music_fresh: bool) -> PartCtx<'_> {
            self.player.seek_tick(frame);
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.copper,
                Some(&mut self.player),
                frame,
                0,
                1,
                exit,
                music_fresh,
            )
        }

        fn ctx(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, false)
        }

        fn init_ctx(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, true)
        }

        fn init_ctx_chain(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, false, false)
        }

        fn ctx_exit(&mut self, frame: u32) -> PartCtx<'_> {
            self.ctx_with(frame, true, false)
        }
    }

    fn started() -> (Rig<Player>, Lnszoom) {
        let mut rig = Rig::new(Player::running(4_000));
        let mut part = Lnszoom::at("../../");
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started_with(player: Player) -> (Rig<Player>, Lnszoom) {
        let mut rig = Rig::new(player);
        let mut part = Lnszoom::at("../../");
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    /// The part with the **real** player: `MUSIC1` seeked to order 45, the
    /// adapter the engine polls once before each tick. This is what a
    /// `--part lnszoom` run hands the part.
    fn started_real() -> (Rig<crate::audio::PlayerMusic>, Lnszoom) {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(45);
        let mut rig = Rig::new(PlayerMusic::new(player, 0));
        let mut part = Lnszoom::at("../../");
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn run_ticks<M: RigMusic + 'static>(rig: &mut Rig<M>, part: &mut Lnszoom, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            rig.player.pre_tick();
            assert_eq!(
                part.frame(&mut rig.ctx(at)),
                PartStatus::Continuing,
                "tick {at}"
            );
        }
    }

    fn run_to<M: RigMusic + 'static>(rig: &mut Rig<M>, part: &mut Lnszoom, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    // --- the three acts' lengths and gates -----------------------------------

    #[test]
    fn the_acts_are_the_trees_lengths_and_gates() {
        assert_eq!(PART1_FRAMES, 300);
        assert_eq!(WIPE_FRAMES, 80);
        assert_eq!(WIPE_COPIES, 6);
        assert_eq!(LENS_FRAMES, 715);
        assert_eq!(LENS_HOLD_END, 720);
        assert_eq!(ROT_FRAMES, 2000);
        assert_eq!(ROT_FADE_START, 1872);
        assert_eq!(ROT_WHITE_STEPS, 16);
        assert_eq!((PART1_GATE, LENS_GATE, ROT_BREAK), (-6, -20, -4));
        assert_eq!(ROT_SETUP_FRAMES, 2);
    }

    #[test]
    fn the_wipe_runs_three_hundred_frames_and_draws_only_the_first_eighty() {
        let (mut rig, mut part) = started();
        // Tick 0 is the gate; the first wipe frame is tick 1.
        run_to(&mut rig, &mut part, 1);
        assert_eq!(part.phase, Phase::Wipe { frame: 0 });
        // The picture is fully painted by frame 79; the screen is the source
        // picture byte for byte.
        run_to(&mut rig, &mut part, 80);
        assert_eq!(part.phase, Phase::Wipe { frame: 79 });
        assert_eq!(
            &rig.surface.pixels()[..VIDEO_BYTES],
            &part.back[..VIDEO_BYTES],
            "the wipe converges on the picture"
        );
        // The loop keeps running to 300 frames with nothing drawn.
        run_to(&mut rig, &mut part, 300);
        assert_eq!(part.phase, Phase::Wipe { frame: 299 });
        run_to(&mut rig, &mut part, 301);
        assert_eq!(part.phase, Phase::LensWait);
    }

    #[test]
    fn the_gates_wait_below_their_thresholds_and_pass_at_them() {
        // Part 1: `while(musplus < -6)` — -7 blocks, -6 passes.
        let mut script = vec![(45, 59, -7); 20];
        for entry in script.iter_mut().skip(5) {
            *entry = (45, 59, -6);
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        run_to(&mut rig, &mut part, 5);
        assert_eq!(part.phase, Phase::Gate, "five ticks at -7");
        run_to(&mut rig, &mut part, 6);
        assert_eq!(
            part.phase,
            Phase::Wipe { frame: 0 },
            "-6 passes at tick 5"
        );
        // The inter-act gate: `while(musplus < -20)` — -21 blocks, -20 passes.
        let mut script = vec![(45, 59, -5); 400];
        for entry in script.iter_mut().skip(301) {
            *entry = (48, 43, -21);
        }
        for entry in script.iter_mut().skip(310) {
            *entry = (48, 44, -20);
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        run_to(&mut rig, &mut part, 310);
        assert_eq!(part.phase, Phase::LensWait, "nine ticks at -21");
        run_to(&mut rig, &mut part, 311);
        assert_eq!(
            part.phase,
            Phase::Lens { uframe: 0, frame: 0 },
            "-20 passes at tick 310"
        );
    }

    #[test]
    fn the_missing_call_in_the_part1_guard_is_preserved() {
        // `if(dis_musplus>-30)` tests the function pointer, which is non-zero,
        // so the wait runs for every input. -32 (`zplus` 0, the DIS's no-plus
        // value) is the one input a corrected `if(musplus > -30)` would skip
        // the wait on; the source waits, and this test fails for a "fixed"
        // guard.
        let mut script = vec![(45, 59, -32); 20];
        for entry in script.iter_mut().skip(3) {
            *entry = (45, 59, -7);
        }
        for entry in script.iter_mut().skip(6) {
            *entry = (45, 59, -6);
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        run_to(&mut rig, &mut part, 3);
        assert_eq!(
            part.phase,
            Phase::Gate,
            "three ticks at -32 must not skip the wait"
        );
        run_to(&mut rig, &mut part, 6);
        assert_eq!(part.phase, Phase::Gate, "three ticks at -7");
        run_to(&mut rig, &mut part, 7);
        assert_eq!(
            part.phase,
            Phase::Wipe { frame: 0 },
            "-6 passes at tick 6, the wait's exit"
        );
    }

    #[test]
    fn the_lens_is_seven_hundred_fifteen_frames_and_a_five_frame_hold() {
        // A scripted player whose `-5` passes the second gate at once: the
        // lens begins the tick after part 1's loop leaves.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 301);
        assert_eq!(part.phase, Phase::LensWait);
        run_to(&mut rig, &mut part, 302);
        assert_eq!(part.phase, Phase::Lens { uframe: 0, frame: 0 });
        // The lens's 715 draws land on ticks 302..1016; the hold then takes
        // five ticks before the setup.
        run_to(&mut rig, &mut part, 1_016);
        assert_eq!(part.phase, Phase::Lens { uframe: 714, frame: 714 });
        run_to(&mut rig, &mut part, 1_017);
        assert_eq!(part.phase, Phase::LensHold { uframe: 715 });
        run_to(&mut rig, &mut part, 1_021);
        assert_eq!(part.phase, Phase::LensHold { uframe: 719 });
        run_to(&mut rig, &mut part, 1_022);
        assert_eq!(part.phase, Phase::RotSetup { stage: 0 });
        run_to(&mut rig, &mut part, 1_024);
        assert_eq!(part.phase, Phase::Rot { frame: 0 });
        // `:200`'s `setpalarea(fade+64*64*3,0,64)` is uploaded and then erased
        // by `inittwk`'s DAC clear: the block is not black, the palette is.
        assert!(
            part.fade[64 * FADE_BLOCK..65 * FADE_BLOCK]
                .iter()
                .any(|value| *value != 0),
            "the erased upload carries a real block"
        );
        assert!(
            (0..=255).all(|index| rig.palette.entry(index) == [0, 0, 0]),
            "`inittwk` leaves the palette black"
        );
        // The rotazoomer's first iteration uploads the first white step
        // (`fade+(64+frame)*64*3` with frame = 1, `:268-271`) to entries 0..63;
        // the rest of the palette stays at `inittwk`'s black.
        run_to(&mut rig, &mut part, 1_025);
        assert_eq!(part.phase, Phase::Rot { frame: 1 });
        for index in 0..64usize {
            let at = 65 * FADE_BLOCK + index * 3;
            assert_eq!(
                rig.palette.entry(index as u8),
                [part.fade[at], part.fade[at + 1], part.fade[at + 2]],
                "entry {index} is the first white step"
            );
        }
        assert_eq!(rig.palette.entry(64), [0, 0, 0]);
    }

    #[test]
    fn the_rotazoomer_is_two_thousand_frames_and_exits_on_the_white_palette() {
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 1_024);
        assert_eq!(part.phase, Phase::Rot { frame: 0 });
        run_to(&mut rig, &mut part, 3_023);
        assert_eq!(part.phase, Phase::Rot { frame: 1999 });
        assert_eq!(
            part.frame(&mut rig.ctx(3_023)),
            PartStatus::Finished,
            "the 2000th iteration ends the loop and the white follows"
        );
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(255), [63, 63, 63]);
        // The finished part keeps the white frame.
        assert_eq!(part.frame(&mut rig.ctx(9_999)), PartStatus::Finished);
    }

    #[test]
    fn the_music_break_ends_the_part_with_the_white_palette() {
        // `MAIN.C:213` `if(dis_musplus()>-4) break;` — the loop's first check
        // of each tick.
        let (mut rig, mut part) = started_with(Player::breaking(1_024, 4_000));
        run_to(&mut rig, &mut part, 1_024);
        assert_eq!(
            part.frame(&mut rig.ctx(1_024)),
            PartStatus::Finished,
            "musplus 0 is above -4"
        );
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(rig.palette.entry(7), [63, 63, 63]);
    }

    #[test]
    fn exit_requested_finishes_the_act_on_that_tick() {
        let (mut rig, mut part) = started();
        assert_eq!(
            part.frame(&mut rig.ctx_exit(0)),
            PartStatus::Finished,
            "the part 1 gate"
        );
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 10);
        assert!(matches!(part.phase, Phase::Wipe { .. }));
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the wipe"
        );
        // Part 1's gate passes and the inter-act gate blocks: `-5` through
        // the wipe, `-32` afterwards.
        let mut script = vec![(45, 59, -5); 400];
        for entry in script.iter_mut().skip(301) {
            *entry = (45, 59, -32);
        }
        let (mut rig, mut part) = started_with(Player::scripted(script));
        run_to(&mut rig, &mut part, 301);
        assert_eq!(part.phase, Phase::LensWait);
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the inter-act gate"
        );
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 400);
        assert!(matches!(part.phase, Phase::Lens { .. }));
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the lens"
        );
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 2_000);
        assert!(matches!(part.phase, Phase::Rot { .. }));
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the rotazoomer"
        );
    }

    // --- the entry pre-roll and the reference's timeline ---------------------

    #[test]
    fn the_entry_pre_roll_lands_the_reference_position_at_tick_zero() {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        assert_eq!(MUSIC_PRE_ROLL, 490);
        // `reference/music/second3/trace.csv` at 2452: 45:59, zplus 1, and
        // `musplus` -5 — the part 1 gate passes without waiting.
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(45);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        assert_eq!(
            (music.position().order, music.position().row),
            (45, 59),
            "the pre-roll reaches the measured row before the engine's tick-0 poll"
        );
        // The engine's tick-0 poll is the next one.
        music.poll();
        assert_eq!(
            (music.position().order, music.position().row),
            (45, 59),
            "the trace's position at boot 2452"
        );
        assert_eq!(music.musplus(), -5, "`musplus < -6` is false: the gate passes");
        // The same pre-roll puts the 48:44 row at tick 473, the trace's boot
        // 2925 — the inter-act gate's frame.
        for _ in 0..473 {
            music.poll();
        }
        assert_eq!(
            (music.position().order, music.position().row),
            (48, 44)
        );
        assert_eq!(music.musplus(), LENS_GATE, "`musplus < -20` is false");
    }

    #[test]
    fn the_real_player_puts_the_acts_on_the_reference_ticks() {
        // Tick 0 = boot 2452; the lens's first draw is tick 474 (boot 2926,
        // path frame 0) and the rotazoomer's tick 1196 (boot 3648, frame 0).
        let (mut rig, mut part) = started_real();
        run_to(&mut rig, &mut part, 473);
        assert_eq!(part.phase, Phase::LensWait, "the gate passes at 473");
        run_to(&mut rig, &mut part, 475);
        assert_eq!(
            part.phase,
            Phase::Lens { uframe: 1, frame: 1 },
            "the lens's frame 0 is drawn on tick 474"
        );
        run_to(&mut rig, &mut part, 1_197);
        assert_eq!(
            part.phase,
            Phase::Rot { frame: 1 },
            "the rotazoomer's frame 0 is drawn on tick 1196"
        );
        assert_eq!(REFERENCE_FIRST_INDEX + 474, 2_926);
        assert_eq!(REFERENCE_FIRST_INDEX + 1_196, 3_648);
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        let mut rig = Rig::new(Player::running(10));
        let mut part = Lnszoom::at("../../");
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player.polls, 0, "a mid-chain start polls nothing");
    }

    #[test]
    fn a_run_without_music_is_refused() {
        let mut rig = Rig::new(Player::running(10));
        let mut part = Lnszoom::at("../../");
        rig.player.seek_tick(0);
        let mut ctx = PartCtx::new(
            &mut rig.surface,
            &mut rig.view,
            &mut rig.palette,
            &mut rig.copper,
            None,
            0,
            0,
            1,
            false,
            true,
        );
        let error = part.init(&mut ctx).expect_err("no music, no gates");
        assert!(error.message.contains("music"), "{}", error.message);
    }

    // --- the assets ----------------------------------------------------------

    fn assets() -> Lnszoom {
        let driver = Lnszoom::at("../../");
        let mut part = Lnszoom::new();
        part.root = "../../";
        part.ex0 = driver.read(EX0_PATH).unwrap();
        let overlays = part.ex0_overlays();
        part.tables = [
            driver.read(EX1_PATH).unwrap(),
            driver.read(EX2_PATH).unwrap(),
            driver.read(EX3_PATH).unwrap(),
            driver.read(EX4_PATH).unwrap(),
        ];
        let image = driver.read_image(EXB_PATH).unwrap();
        part.back = vec![0u8; VIDEO_MEMORY];
        part.back[..VIDEO_BYTES].copy_from_slice(&image.pixels()[..VIDEO_BYTES]);
        let from = VIDEO_BYTES - OVERRUN_BYTES;
        let tail = part.back[from..VIDEO_BYTES].to_vec();
        part.back[VIDEO_BYTES..].copy_from_slice(&tail);
        part.palette = part.build_palette(&image.palette, overlays);
        let (fade, fade2) = part.build_fades();
        part.fade = fade;
        part.fade2 = fade2;
        part.rotpic = Lnszoom::build_rotpic(&part.back);
        let exp = driver.read(EXP_PATH).unwrap();
        let count = usize::from(u16::from_le_bytes([exp[2], exp[3]]));
        let at = PATH_HEADER;
        part.path1 = (0..2 * PATH1_FRAMES)
            .map(|i| i16::from_le_bytes([exp[at + 2 * i], exp[at + 2 * i + 1]]))
            .collect();
        let at = PATH_HEADER + 2 * count;
        part.path2 = (0..4 * PATH2_FRAMES)
            .map(|i| i16::from_le_bytes([exp[at + 2 * i], exp[at + 2 * i + 1]]))
            .collect();
        part
    }

    #[test]
    fn the_tables_are_bounded_and_the_overrun_is_the_1536_byte_copy() {
        let part = assets();
        assert_eq!(part.back.len(), VIDEO_MEMORY);
        assert_eq!(
            &part.back[VIDEO_BYTES..],
            &part.back[VIDEO_BYTES - OVERRUN_BYTES..VIDEO_BYTES],
            "`MAIN.C:302` copies the last 1,536 bytes past the picture"
        );
        assert_eq!(
            &part.back[..VIDEO_BYTES],
            &part.picture(),
            "the picture itself is LENS.EXB's 64,000 pixel bytes at +784"
        );
        let raw = Lnszoom::at("../../").read(EXB_PATH).unwrap();
        assert_eq!(
            &raw[EXB_PICTURE_AT..EXB_PICTURE_AT + VIDEO_BYTES],
            &part.picture()[..],
            "the decoder's pixels are the file's bytes at +784"
        );
        // EX0's dimensions.
        assert_eq!(
            i16::from_le_bytes([part.ex0[0], part.ex0[1]]),
            LENS_WIDTH as i16
        );
        assert_eq!(
            i16::from_le_bytes([part.ex0[2], part.ex0[3]]),
            LENS_HEIGHT as i16
        );
        // The row tables: 116 entries, the data area at 480, and every row
        // the blitter reads inside its file.
        let mut used = [0usize; 4];
        for (which, drawn) in used.iter_mut().enumerate() {
            let table = &part.tables[which];
            let mut max_end = 0usize;
            for y in 0..LENS_ROWS {
                let (at, count) = part.lens_row(which, y);
                let reads = if which == 0 { count >= 4 } else { count > 0 };
                if reads {
                    *drawn += 1;
                    assert!(at >= LENS_DATA_BASE, "row {y}'s data before 480");
                    let end = match which {
                        1 | 2 => at + 2 + 4 * count,
                        _ => at + 2 + 2 * count,
                    };
                    assert!(end <= table.len(), "row {y} runs past its file");
                    max_end = max_end.max(end);
                }
            }
            assert!(max_end <= table.len(), "table {which} runs past its file");
            println!(
                "EX{}: {} bytes, {} rows drawn, data ends at {}",
                which + 1,
                table.len(),
                drawn,
                max_end
            );
        }
        assert_eq!(used[0], 103, "EX1's drawn rows");
        assert_eq!(used[1], 102, "EX2's drawn rows");
        assert_eq!(used[2], 43, "EX3's drawn rows");
        assert_eq!(used[3], 116, "EX4's drawn rows");
    }

    impl Lnszoom {
        fn picture(&self) -> Vec<u8> {
            self.back[..VIDEO_BYTES].to_vec()
        }
    }

    #[test]
    fn the_palette_and_fades_are_the_sources_arithmetic() {
        let part = assets();
        let image = Lnszoom::at("../../").read_image(EXB_PATH).unwrap();
        // The overlays add EX0's triples to the base, clamped at 63.
        assert_eq!(part.ex0_overlays(), EXPECTED_OVERLAYS);
        for (i, (r, g, b)) in part.ex0_overlays().iter().enumerate() {
            for entry in 0..64 {
                let base = image.palette.entry(entry as u8);
                let at = (i + 1) * 64 * 3 + entry * 3;
                for (channel, delta) in [*r, *g, *b].iter().enumerate() {
                    let want = (base[channel] + delta).min(63);
                    assert_eq!(part.palette[at + channel], want, "bank {} entry {entry}", i + 1);
                }
            }
        }
        // The first fade block is the base palette; the last of the 64 is
        // white; the 16 extra blocks run `pal + (15-x)*5`.
        assert_eq!(&part.fade[..FADE_BLOCK], &part.palette[..FADE_BLOCK]);
        assert!(part.fade[63 * FADE_BLOCK..64 * FADE_BLOCK]
            .iter()
            .all(|v| *v == 63));
        assert_eq!(&part.fade[79 * FADE_BLOCK..80 * FADE_BLOCK], &part.palette[..FADE_BLOCK]);
        assert_eq!(part.fade.len(), FADE_BLOCKS * FADE_BLOCK);
        assert_eq!(part.fade2.len(), FADE2_BLOCKS * FADE2_BLOCK);
        // fade2 block 0 at entry 64+k is the overlay's difference from the
        // base; block 31 is the overlay itself.
        let base = &part.palette[..64 * 3];
        assert_eq!(
            part.fade2[0],
            part.palette[64 * 3].wrapping_sub(base[0]),
        );
        assert_eq!(
            &part.fade2[31 * FADE2_BLOCK..32 * FADE2_BLOCK],
            &part.palette[64 * 3..256 * 3]
        );
    }

    #[test]
    fn the_path_data_is_the_expected_size_and_starts_where_the_c_prelude_reads() {
        let part = assets();
        let exp = Lnszoom::at("../../").read(EXP_PATH).unwrap();
        let count = usize::from(u16::from_le_bytes([exp[2], exp[3]]));
        assert_eq!(count, 2 * PATH1_FRAMES);
        assert_eq!(PATH_HEADER + 2 * count + PATH2_FRAMES * 8, exp.len());
        assert_eq!(part.path1.len(), 2 * PATH1_FRAMES);
        assert_eq!(part.path2.len(), 4 * PATH2_FRAMES);
        assert_eq!(&part.path1[..6], &[65, -50, 66, -49, 67, -47]);
        assert_eq!(&part.path2[..8], &[-30, 3, 0, 2047, -30, 2, 0, 2037]);
    }

    #[test]
    fn rotpic_is_the_squashed_face_and_rotpic90_its_rotation() {
        let part = assets();
        // `a=y*10/11-18`, clamped to row 0 outside 0..199.
        for (x, y) in [(0usize, 0usize), (37, 20), (255, 235), (0, 255), (13, 240)] {
            let mut a = y as i32 * 10 / 11 - 18;
            if !(0..=199).contains(&a) {
                a = 0;
            }
            assert_eq!(part.rotpic[x + y * 256], part.back[x + 32 + a as usize * 320]);
        }
        let rotated = Lnszoom::build_rotpic90(&part.rotpic);
        for (x, y) in [(0usize, 0usize), (255, 0), (0, 255), (255, 255), (89, 173)] {
            assert_eq!(rotated[x + y * 256], part.rotpic[y + (255 - x) * 256]);
        }
    }

    // --- the rotazoomer ------------------------------------------------------

    /// A synthetic part: no assets, a texture whose bytes are their address
    /// modulo 251, and one path frame.
    fn synthetic(path: [i16; 4]) -> Lnszoom {
        let mut part = Lnszoom::new();
        part.root = "../../";
        part.rotpic = (0..VIDEO_MEMORY).map(|i| (i % 251) as u8).collect();
        part.rotpic90 = Lnszoom::build_rotpic90(&part.rotpic);
        part.path2 = path.to_vec();
        part.video = vec![0u8; VIDEO_MEMORY];
        part.phase = Phase::Rot { frame: 0 };
        part
    }

    /// The sample the source's row `row`, step `n` reads, derived from
    /// `ASM.ASM` independently of [`Lnszoom::rotate`]: the displacement table
    /// uses the uncorrected vector, the row base the 307/256-corrected one.
    fn expected_samples(part: &Lnszoom, frame: usize, row: usize) -> Vec<u8> {
        let x = i32::from(part.path2[frame * 4]);
        let y = i32::from(part.path2[frame * 4 + 1]);
        let xa = i32::from(part.path2[frame * 4 + 2]);
        let ya = i32::from(part.path2[frame * 4 + 3]);
        let mut xadd = xa << 6;
        let mut yadd = ya << 6;
        let mut xpos = x << 16;
        let mut ypos = y << 16;
        let rotated = xadd.unsigned_abs() > yadd.unsigned_abs();
        if rotated {
            std::mem::swap(&mut xadd, &mut yadd);
            xadd = -xadd;
            let old = (xpos, ypos);
            xpos = -old.1;
            ypos = old.0;
        }
        let mut disp = Vec::new();
        for n in 1..=160i32 {
            let al = ((n.wrapping_mul(yadd) as u32) >> 16) as u8;
            let ah = ((n.wrapping_mul(-xadd) as u32) >> 16) as u8;
            disp.push(u16::from(ah) << 8 | u16::from(al));
        }
        let aspect = |v: i32| (307u32.wrapping_mul(v as u32) as i32) >> 8;
        let (xadd, yadd) = (aspect(xadd), aspect(yadd));
        for _ in 0..=row {
            ypos = ypos.wrapping_add(yadd);
            xpos = xpos.wrapping_add(xadd);
        }
        let base = (((ypos as u32 >> 16) & 0xff) << 8 | ((xpos as u32 >> 16) & 0xff)) as usize;
        let source = if rotated { &part.rotpic90 } else { &part.rotpic };
        disp.iter()
            .map(|step| source[(base + usize::from(*step)) & 0xffff])
            .collect()
    }

    fn one_frame(part: &Lnszoom) -> Vec<u8> {
        let mut rig = Rig::new(Player::running(1));
        let mut part = clone_for_rotate(part);
        let status = part.frame(&mut rig.ctx(0));
        assert_eq!(status, PartStatus::Continuing);
        rig.surface.pixels()[..VIDEO_BYTES].to_vec()
    }

    fn clone_for_rotate(part: &Lnszoom) -> Lnszoom {
        Lnszoom {
            root: part.root,
            ex0: Vec::new(),
            tables: part.tables.clone(),
            path1: part.path1.clone(),
            path2: part.path2.clone(),
            back: Vec::new(),
            palette: Vec::new(),
            fade: vec![0u8; FADE_BLOCKS * FADE_BLOCK],
            fade2: vec![0u8; FADE2_BLOCKS * FADE2_BLOCK],
            rotpic: part.rotpic.clone(),
            rotpic90: part.rotpic90.clone(),
            video: vec![0u8; VIDEO_MEMORY],
            firfade1: Vec::new(),
            firfade2: Vec::new(),
            firfade1a: Vec::new(),
            firfade2a: Vec::new(),
            phase: Phase::Rot { frame: 0 },
            tick: 0,
        }
    }

    #[test]
    fn the_rotazoomer_composes_160_samples_each_shown_twice_and_100_rows_twice() {
        let part = synthetic([5, 7, 300, 1200]);
        let plane = one_frame(&part);
        assert_eq!(plane.len(), VIDEO_BYTES);
        // Every character row appears on two scanlines.
        for row in 0..ROT_ROWS {
            assert_eq!(
                &plane[2 * row * 320..(2 * row + 1) * 320],
                &plane[(2 * row + 1) * 320..(2 * row + 2) * 320],
                "row {row}"
            );
        }
        // Each sample is two pixels.
        for row in 0..ROT_ROWS {
            let want = expected_samples(&part, 0, row);
            for (n, value) in want.iter().enumerate() {
                assert_eq!(plane[2 * row * 320 + 2 * n], *value, "row {row} step {n}");
                assert_eq!(plane[2 * row * 320 + 2 * n + 1], *value);
            }
        }
    }

    #[test]
    fn the_aspect_ratio_scales_the_row_advance_not_the_in_row_steps() {
        // With `xa=0, ya=0` the row advance is zero and every row is the same;
        // with a non-zero `ya` the rows differ. The displacement table itself
        // is the uncorrected vector: sample 160's address is built from
        // 160*ya<<6 when the row corrects by 160*(307*ya<<6)>>8.
        let _part = synthetic([0, 0, 0, 400]);
        let disp = Lnszoom::displacements(0, 400 << 6);
        assert_eq!(
            disp[159],
            (((160i64 * (400 << 6)) as u32 >> 16) as u8) as u16,
            "the table keeps the uncorrected step"
        );
        let aspect = (307u32.wrapping_mul(400 << 6) as i32) >> 8;
        assert_eq!(aspect, (400 << 6) * 307 / 256);
        assert_ne!(aspect, 400 << 6);
    }

    #[test]
    fn the_axis_swap_selects_rotpic90_and_the_swapped_walk() {
        // |xa| > |ya| takes the rotated texture; the composed samples must
        // come from it, not from rotpic.
        let part = synthetic([0, 0, 900, 100]);
        let plane = one_frame(&part);
        let want = expected_samples(&part, 0, 37);
        let mut want_row = Vec::new();
        for value in &want {
            want_row.push(*value);
            want_row.push(*value);
        }
        assert_eq!(&plane[2 * 37 * 320..2 * 37 * 320 + 320], want_row.as_slice());
        // A different texture under the two names proves which one was read.
        let mut other = clone_for_rotate(&part);
        other.rotpic90.iter_mut().for_each(|v| *v = 0xAA);
        let plane2 = one_frame(&other);
        assert!(plane2.contains(&0xAA), "rotpic90 is the source");
        assert_ne!(plane, plane2);
        // |xa| <= |ya| takes rotpic.
        let part = synthetic([0, 0, 100, 900]);
        let plane = one_frame(&part);
        assert_ne!(plane, plane2);
    }

    /// The displayed frame as RGB, through the live palette and the engine's
    /// 6→8-bit widening — what `--dump-frames` writes.
    fn rendered_rgb(rig: &Rig<Player>) -> Vec<u8> {
        let mut out = Vec::with_capacity(VIDEO_BYTES * 3);
        for y in 0..SCREEN_HEIGHT {
            for x in 0..SCREEN_WIDTH {
                // The part composes a linear 320×200 picture; this helper is
                // used on clones that never ran `init`, so it reads the same
                // linear layout rather than a view those clones do not set.
                let at = y as usize * SCREEN_WIDTH as usize + x as usize;
                let entry = rig.palette.entry(rig.surface.pixels()[at]);
                out.push(dac_expand(entry[0]));
                out.push(dac_expand(entry[1]));
                out.push(dac_expand(entry[2]));
            }
        }
        out
    }

    fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
        assert_eq!(rendered.len(), expected.len());
        rendered
            .chunks_exact(3)
            .zip(expected.chunks_exact(3))
            .filter(|(a, b)| a != b)
            .count()
    }

    fn reference(index: u32) -> Vec<u8> {
        std::fs::read(format!(
            "../../reference/parts/lnszoom/frame_{index:06}.rgb"
        ))
        .unwrap()
    }

    /// The D24 divergence and its proof: the tree's picture differs from the
    /// release's in 2,058 forehead pixels, and patching those bytes makes the
    /// lens's 3400 and the rotazoomer's 4200/4900 byte-identical.
    #[test]
    fn the_forehead_asset_difference_is_the_whole_residual() {
        let mut patched = assets();
        let reference_plane =
            std::fs::read("../../reference/parts/lnszoom/frame_002700.idx").unwrap();
        let differing: Vec<usize> = (0..VIDEO_BYTES)
            .filter(|i| patched.back[*i] != reference_plane[*i])
            .collect();
        assert_eq!(differing.len(), 2058, "the forehead's pixels");
        let rows: Vec<usize> = differing.iter().map(|i| i / 320).collect();
        let cols: Vec<usize> = differing.iter().map(|i| i % 320).collect();
        assert_eq!(
            (rows.iter().min(), rows.iter().max()),
            (Some(&33), Some(&91)),
            "the differing rows"
        );
        assert_eq!(
            (cols.iter().min(), cols.iter().max()),
            (Some(&129), Some(&211)),
            "the differing columns"
        );
        for &at in &differing {
            patched.back[at] = reference_plane[at];
        }
        let tail = patched.back[VIDEO_BYTES - OVERRUN_BYTES..VIDEO_BYTES].to_vec();
        patched.back[VIDEO_BYTES..].copy_from_slice(&tail);
        patched.rotpic = Lnszoom::build_rotpic(&patched.back);
        patched.rotpic90 = Lnszoom::build_rotpic90(&patched.rotpic);

        // The lens at its 3400 frame (path frame 474): the patched picture
        // behind the bubble.
        let mut lens = clone_for_rotate(&patched);
        lens.back = patched.back.clone();
        lens.palette = patched.palette.clone();
        lens.video[..VIDEO_BYTES].copy_from_slice(&patched.back[..VIDEO_BYTES]);
        lens.phase = Phase::Lens { uframe: 474, frame: 474 };
        let mut rig = Rig::new(Player::running(1_000));
        rig.palette = lens.live_palette();
        assert_eq!(lens.frame(&mut rig.ctx(474)), PartStatus::Continuing);
        assert_eq!(
            differing_pixels(&rendered_rgb(&rig), &reference(3_400)),
            0,
            "the patched picture alone accounts for 3400's residual"
        );

        // The rotazoomer's two keyframes.
        for (frame, index) in [(552usize, 4_200u32), (1_252, 4_900)] {
            let mut part = clone_for_rotate(&patched);
            part.palette = patched.palette.clone();
            part.phase = Phase::Rot { frame: frame as u32 };
            let mut rig = Rig::new(Player::running(2_000));
            rig.palette = part.live_palette();
            assert_eq!(
                part.frame(&mut rig.ctx(frame as u32)),
                PartStatus::Continuing
            );
            assert_eq!(
                differing_pixels(&rendered_rgb(&rig), &reference(index)),
                0,
                "the patched picture alone accounts for {index}'s residual"
            );
        }
    }

    #[test]
    fn rotatez_is_dead() {
        // The tree's `rotatez` (`ASM.ASM:419-564`) differs only by the two
        // adds in its row loop and is never called; the port has no such path.
        let source = std::fs::read_to_string("../../upstream/SecondReality/LENS/ASM.ASM").unwrap();
        assert!(source.contains("_rotatez PROC FAR"));
        assert_eq!(
            source.matches("_rotatez").count(),
            3,
            "declared PUBLIC, opened and ended — never called"
        );
        assert!(!source.contains("call\t_rotatez") && !source.contains("call _rotatez"));
    }
}
