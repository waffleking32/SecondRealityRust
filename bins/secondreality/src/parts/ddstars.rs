//! DDSTARS — the hidden part: a 3-D starfield under a rising text wipe.
//!
//! `DDSTARS/KOE.ASM` (the program) `include`s `DDSTARS/STARS.ASM` (the
//! effect); the linked `POLYEGA.ASM` is never called. A tick is one
//! `do_stars` iteration:
//!
//! ```text
//! init   KOE.ASM:72-76     resetmode13: mode 13h, AC 0-15 identity, overscan 255, DAC black
//!        STARS.ASM:56-176  init_stars: CRTC 9 bit 7 cleared, video and clearsbu's 128 8 KB
//!                          units zeroed (the ring uses 64), the star and muldiv tables, 100
//!                          starfetch0/staradd passes
//! tick   :540-770          flip, waitb, the fade while starpalfade <= 32, starframe's marks,
//!                          risetext into the displayed page, a star pass, the blit into the
//!                          back page; starframe 901-1200 draws no star (:670-676)
//! exit   :772-776          DIS function 2 only: no cap
//! ```
//!
//! The screen is 320×400 with a 40-byte plane row. The star blits write every
//! other row (80-byte steps, `:697-770`) and `risetext` starts 40 bytes into
//! the stride (`:450-460`), so stars are on even rows and text on odd ones;
//! the text's set/reset forces plane 2 (`:479-483`), so its colours are 4-7.
//! Because the text goes to the displayed page and the stars to the back
//! page (`:667` against `:683-684`), the screen at tick `t` carries the text
//! of `t` and the stars of `t-1`.
//!
//! The 64 ring units are cleared once and then only ORed into, so the field
//! accumulates; `staradd2` writes each bit to unit `u` and `(u + 17) & 63`
//! (`starfetch1`), and the bottom half shows unit `(u + 32) & 63` reversed.
//! The stars are not clipped under the text: the tree's `_nostar` guards are
//! commented out and the keyframes hold the stars they would skip (D24).
//!
//! The part makes no music calls (the shell restarts `MUSIC0` at order 70,
//! `MAIN/U2.ASM:889-894`). The mapping to the reference is boot = 414 + tick,
//! not an anchor: `docs/fidelity/ddstars.md` §3. Preserved bugs: `bugs.md` §27.

use std::sync::OnceLock;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{load_image, mode_view};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "ddstars";

// ------------------------------------------------------------- the screen --

/// Mode 13h's CRTC offset is never written, so a plane row is 40 bytes.
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 400;
const PAGE_ROW_BYTES: usize = 40;
/// One page, one plane. The pages at 0 and 0x4000 do not overlap.
const PAGE_BYTES: usize = SCREEN_HEIGHT * PAGE_ROW_BYTES;
/// The star blit advances the destination by 80 bytes a source row
/// (`STARS.ASM:697-704`), so it lands on every other page row.
const BLIT_ROW_STRIDE: usize = PAGE_ROW_BYTES * 2;
/// The text's writes start 40 bytes into the 80-byte stride (`:450-460`).
const TEXT_OFFSET: usize = 40;

// --------------------------------------------------------------- the ring --

/// `STARS.ASM:163-164` `emmpage4`'s mask: 64 units, allocated 8 KB each.
const RING_UNITS: usize = 64;
/// One unit's bytes: 100 rows × 40 bytes of plane 0, a gap, then the same for
/// plane 1 at +4096 (`:315-327`). 8 KB, the EMS unit.
const UNIT_BYTES: usize = 8192;
/// The plane-1 offset inside a unit (`bx+4096`, `:324`).
const UNIT_PLANE1: usize = 4096;
/// The unit written in the same pass as the fetched one: `(u + 17) & 63`
/// (`starfetch1`, `:260-267`).
const UNIT_SECOND: usize = 17;
/// The unit the bottom half of the screen reads: `(u + 32) & 63`
/// (`:726-729`), reversed.
const UNIT_MIRROR: usize = 32;
/// The part's first star write is unit `emmpage4` after `init_stars`'s 100
/// `starfetch0` calls: 100 & 63 = 36.
const UNIT_AFTER_INIT: u8 = 100 % RING_UNITS as u8;

// --------------------------------------------------------------- the stars --

/// `STARS2 equ 1024`: the table, and the entries `staradd2` walks.
const STAR_COUNT: usize = 1024;
/// `STARS equ 512`: the entries `staradd` walks, and `starlimit`'s start.
const STAR_ADD: usize = 512;
/// The 100 `starfetch0`/`staradd` passes `init_stars` runs before the first
/// displayed frame (`:169-174`).
const INIT_PASSES: u32 = 100;

// ---------------------------------------------------------------- the text --

/// `_textpic`, linked from `DDSTARS/TEXTS.16` (`MAKEFILE:2`, `DOTEXTS.BAT`).
/// The Origin decoder's plane path reads it as the two 1bpp planes it is.
const TEXT_PATH: &str = "assets/DDSTARS/TEXTS.16";
const TEXT_WIDTH: usize = 320;
const TEXT_HEIGHT: usize = 200;
/// One source text row: two 40-byte planes, the layout `risetext` reads.
const TEXT_ROW: usize = 80;
/// The marks' two page offsets: `:655` `mov startxtp0,80` and `:660`
/// `mov startxtp0,101*80`.
const TEXT_BLOCK0: u16 = 80;
const TEXT_BLOCK1: u16 = 101 * 80;
/// `:656-657` / `:661-662`: the wipe's state at each mark.
const TEXT_OPEN_START: i16 = -256;
const TEXT_CLOSE_START: i16 = 1500;

// -------------------------------------------------------------- the clock --

/// `:653-665`: the stage marks, on `starframe` after its increment.
const MARK_TEXT0: u16 = 1200;
const MARK_TEXT1: u16 = 3200;
const MARK_STARS: u16 = 1500;
/// `:670-676`: no star pass is made between these, exclusive.
const STARS_PAUSE_FROM: u16 = 900;
const STARS_PAUSE_TO: u16 = 1200;

// ---------------------------------------------------------------- the RNG --

/// `STARS.ASM:33-42`'s `random`: a 32-bit LCG from seed 0 (`:32`) whose
/// return is the `mul`'s `dx`, bits 32-47 of the product, not the new seed's
/// high word (the add does not carry into `edx`).
struct Lcg {
    seed: u32,
}

impl Lcg {
    const MULTIPLIER: u32 = 0x0003_43FD;
    const INCREMENT: u32 = 0x0026_9EC3;

    fn new() -> Lcg {
        Lcg { seed: 0 }
    }

    fn next(&mut self) -> u16 {
        let product = u64::from(Self::MULTIPLIER) * u64::from(self.seed);
        self.seed = (product as u32).wrapping_add(Self::INCREMENT);
        (product >> 32) as u16
    }
}

// ---------------------------------------------------------- the projection --

/// `STARS.ASM:141-161`: the two 256-entry tables, `[y, x]`, built once:
/// `((n << 16) / (150 + 4z)) >> 1` with `n` 108 for y and 144 for x.
fn muldiv_tables() -> &'static [[u16; 256]; 2] {
    static TABLES: OnceLock<[[u16; 256]; 2]> = OnceLock::new();
    TABLES.get_or_init(|| {
        [108u32, 144].map(|numerator| {
            std::array::from_fn(|z| (((numerator << 16) / (150 + 4 * z as u32)) >> 1) as u16)
        })
    })
}

/// `STARS.ASM:294-313`: the product shifted right 14 (`shld dx,ax,2`), the
/// 16-bit add and the unsigned `ja @@2`, so `Some((row, column))` only for
/// rows 0-99 and columns 0-319.
fn project(z: u8, x: i16, y: i16) -> Option<(usize, usize)> {
    let [muldiv_y, muldiv_x] = muldiv_tables();
    let row = ((i32::from(y) * i32::from(muldiv_y[usize::from(z)])) >> 14) as u16;
    let row = row.wrapping_add(100);
    if row > 99 {
        return None;
    }
    let column = ((i32::from(x) * i32::from(muldiv_x[usize::from(z)])) >> 14) as u16;
    let column = column.wrapping_add(160);
    if column > 319 {
        return None;
    }
    Some((row as usize, column as usize))
}

/// The z bands `STARS.ASM:317-327` select: `(plane0, plane1)`.
fn star_planes(z: u8) -> (bool, bool) {
    if z >= 180 {
        (true, false)
    } else if z >= 110 {
        (false, true)
    } else {
        (true, true)
    }
}

// ------------------------------------------------------------- the palette --

/// `STARS.ASM:559-648`: the eight entries the fade writes, at `b` from the
/// shift. Entries 1-3 are the scaled colours; 4 is black; 5-7 are fixed.
fn fade_palette(b: u8) -> Palette {
    let scaled = |component: u32| ((component * u32::from(b)) >> 8) as u8;
    let mut dac = Palette::new();
    dac.set_entry(0, [0, 0, 0]);
    // `25*70/100`, `31*70/100`, `38*70/100` = 17, 21, 26.
    dac.set_entry(1, [scaled(17), scaled(21), scaled(26)]);
    // `45*56/100`, `58*56/100`, `69*56/100` = 25, 32, 38.
    dac.set_entry(2, [scaled(25), scaled(32), scaled(38)]);
    // `67*64/100`, `84*64/100`, `99*64/100` = 42, 53, 63.
    dac.set_entry(3, [scaled(42), scaled(53), scaled(63)]);
    dac.set_entry(4, [0, 0, 0]);
    dac.set_entry(5, [10, 20, 35]);
    dac.set_entry(6, [20, 30, 45]);
    dac.set_entry(7, [30, 40, 60]);
    dac
}

// --------------------------------------------------------------- the part --

/// One entry of `star dw STARS2 dup(0,0,0,0)` (`STARS.ASM:26`). `z` is a
/// byte: the code writes only its low byte (`:290`) and reads it by `movzx`.
#[derive(Clone, Copy, Default)]
struct Star {
    z: u8,
    x: i16,
    y: i16,
}

/// One page: the three planes the part writes (plane 3 stays 0) and the
/// composed 320×400 indices, recomposed by [`Page::compose_row`] for every
/// row a write touches; [`Ddstars::present`] copies them.
struct Page {
    plane0: Vec<u8>,
    plane1: Vec<u8>,
    plane2: Vec<u8>,
    index: Vec<u8>,
}

impl Page {
    fn new() -> Page {
        Page {
            plane0: vec![0; PAGE_BYTES],
            plane1: vec![0; PAGE_BYTES],
            plane2: vec![0; PAGE_BYTES],
            index: vec![0; SCREEN_WIDTH * SCREEN_HEIGHT],
        }
    }

    /// Recompose one 320-pixel row from the page's planes.
    fn compose_row(&mut self, row: usize) {
        let table = pair_table();
        let source = row * PAGE_ROW_BYTES;
        let destination = row * SCREEN_WIDTH;
        for byte in 0..PAGE_ROW_BYTES {
            let index = (usize::from(self.plane0[source + byte]) << 8)
                | usize::from(self.plane1[source + byte]);
            let start = destination + byte * 8;
            self.index[start..start + 8].copy_from_slice(&table[index]);
            let plane2 = self.plane2[source + byte];
            if plane2 != 0 {
                for bit in 0..8 {
                    if plane2 & (0x80 >> bit) != 0 {
                        self.index[start + bit] |= 4;
                    }
                }
            }
        }
    }

    /// Clear `bytes` bytes of all three planes at `at`, and recompose the
    /// rows the clear covered. `at` and `bytes` are multiples of 40.
    fn clear(&mut self, at: usize, bytes: usize) {
        self.plane0[at..at + bytes].fill(0);
        self.plane1[at..at + bytes].fill(0);
        self.plane2[at..at + bytes].fill(0);
        for row in at / PAGE_ROW_BYTES..(at + bytes) / PAGE_ROW_BYTES {
            self.compose_row(row);
        }
    }
}

/// DDSTARS: the star ring, the two pages, the star table and the text strip.
pub struct Ddstars {
    stars: Vec<Star>,
    /// The 64 EMS units, plane 0 at +0 and plane 1 at +4096. Never cleared
    /// after `init`; that persistence is the picture.
    ring: Vec<u8>,
    /// `starvram`'s two pages. The CRTC shows the one not being written.
    pages: [Page; 2],
    /// `_textpic`'s two 1bpp planes, one 40-byte row each per source row.
    text: [Vec<u8>; 2],
    /// `emmpage4` (`:13`): the unit `starfetch0` last fetched.
    emmpage4: u8,
    /// `starlimit` (`:14`): the count gate `:281-286` / `:347-351`.
    starlimit: u16,
    /// `starpalfade` (`:19`): 0 through 33; the fade runs while it is ≤ 32.
    starpalfade: u8,
    /// `starframe` (`:18`): the marks' counter, one a tick from 1.
    starframe: u16,
    /// `startxtopen/close/p0` (`:15-17`): the wipe's state.
    startxtopen: i16,
    startxtclose: i16,
    startxtp0: u16,
    rng: Lcg,
    /// Completed `do_stars` iterations: the port's tick, and the page parity.
    tick: u32,
    /// The exit flag was seen; the loop has ended.
    finished: bool,
}

impl Ddstars {
    /// The state `init_stars` starts from; `init` loads the text and the ring.
    pub fn new() -> Ddstars {
        Ddstars {
            stars: Vec::new(),
            ring: Vec::new(),
            pages: [Page::new(), Page::new()],
            text: [Vec::new(), Vec::new()],
            emmpage4: 0,
            starlimit: STAR_ADD as u16,
            starpalfade: 0,
            starframe: 0,
            startxtopen: -9999,
            startxtclose: 10000,
            startxtp0: 0,
            rng: Lcg::new(),
            tick: 0,
            finished: false,
        }
    }

    /// `:123-139`: the 1024-entry table. `z` runs `(cl - 1) & 0xFF` for
    /// `cx` 1024 down to 1 — four descending sweeps — and `x`/`y` are
    /// `random() & 1023 - 512`.
    fn initial_stars(rng: &mut Lcg) -> Vec<Star> {
        let mut stars = Vec::with_capacity(STAR_COUNT);
        for cx in (1..=STAR_COUNT).rev() {
            let z = (cx as u8).wrapping_sub(1);
            let x = (rng.next() & 1023) as i32 - 512;
            let y = (rng.next() & 1023) as i32 - 512;
            stars.push(Star {
                z,
                x: x as i16,
                y: y as i16,
            });
        }
        stars
    }

    /// `starfetch0` (`:250-258`): the next unit, incrementing `emmpage4`.
    fn starfetch0(&mut self) -> u8 {
        self.emmpage4 = (self.emmpage4 + 1) & 63;
        self.emmpage4
    }

    /// One star pass, ORed into every unit in `units`.
    ///
    /// `staradd` passes one unit and 512 entries; `staradd2` passes `u` and
    /// `(u + 17) & 63` and 1024 entries, and lowers `starlimit` by 4
    /// (`:347-351`). The same bits go to every unit because the original's
    /// loop writes `es` and `fs` in one pass (`:378-397`).
    fn star_pass(&mut self, units: &[u8], two: bool) {
        let entries = if two { STAR_COUNT } else { STAR_ADD };
        let mut limit = self.starlimit;
        if two {
            if limit != 0 {
                limit = limit.wrapping_sub(4);
            }
        } else {
            limit = limit.saturating_sub(1);
        }

        let stars = &mut self.stars;
        let rng = &mut self.rng;
        let ring = &mut self.ring;
        for (index, star) in stars.iter_mut().enumerate().take(entries) {
            let bp = (entries - index) as u16;
            let old_z = star.z;
            let z = old_z.wrapping_sub(2);
            star.z = z;
            // `sub bl,2; jc @@4`: the byte wrapped, so respawn x and y.
            if old_z < 2 {
                star.x = ((rng.next() & 1023) as i32 - 512) as i16;
                star.y = ((rng.next() & 1023) as i32 - 512) as i16;
                continue;
            }
            // `cmp bp,starlimit; jb @@2`.
            if bp < limit {
                continue;
            }
            let Some((row, column)) = project(z, star.x, star.y) else {
                continue;
            };
            let offset = row * PAGE_ROW_BYTES + column / 8;
            let bit = 0x80u8 >> (column & 7);
            let (plane0, plane1) = star_planes(z);
            for &unit in units {
                let base = unit as usize * UNIT_BYTES;
                if plane0 {
                    ring[base + offset] |= bit;
                }
                if plane1 {
                    ring[base + UNIT_PLANE1 + offset] |= bit;
                }
            }
        }
        self.starlimit = limit;
    }

    /// `STARS.ASM:413-538`: `risetext`, into the displayed page.
    ///
    /// The `ax` here is the original's `startxtuse` after the two counters
    /// step; the rows it clears and copies are the page's odd rows, because
    /// the whole routine starts 40 bytes into the 80-byte stride.
    fn risetext(&mut self, page: usize) {
        if self.startxtopen < 99 {
            self.startxtopen += 1;
        }
        if self.startxtclose > 0 {
            self.startxtclose -= 1;
        }
        let use_rows = self.startxtopen.min(self.startxtclose);
        if use_rows <= 0 {
            return;
        }
        let rows = if use_rows <= 1 { 2 } else { use_rows };

        let (text0, text1) = (&self.text[0], &self.text[1]);
        let pg = &mut self.pages[page];

        let mut at = 80 * (150 - rows as usize) - TEXT_OFFSET;
        pg.clear(at, PAGE_ROW_BYTES);
        at += 80;
        let mut cx = rows as usize - 1;
        cx -= 1;
        if cx == 0 {
            // `@@tc0c`: two rows cleared by one 80-byte store.
            pg.clear(at, 80);
            return;
        }
        pg.clear(at, PAGE_ROW_BYTES);
        at += 80;
        cx -= 1;
        if cx == 0 {
            // `@@tc0`.
            return;
        }
        cx -= 1;
        if cx == 0 {
            // `@@tc0b`: the copy loop is skipped; one row is cleared.
            pg.clear(at, PAGE_ROW_BYTES);
            return;
        }

        // `@@tc1`: the text rows. Each source row is 80 bytes: plane 0's
        // 40 then plane 1's; the plane mask 0x0D/0x0E writes also force
        // plane 2 = 1 and plane 3 = 0 through the set/reset registers.
        // `si = 040h + startxtp0`: `0x40` is the pixel data and a row is 80
        // bytes, so `startxtp0 / 80` is the 0-based source row.
        let mut source = self.startxtp0 as usize / TEXT_ROW;
        while cx != 0 {
            let src = source * PAGE_ROW_BYTES;
            pg.plane0[at..at + PAGE_ROW_BYTES]
                .copy_from_slice(&text0[src..src + PAGE_ROW_BYTES]);
            pg.plane1[at..at + PAGE_ROW_BYTES]
                .copy_from_slice(&text1[src..src + PAGE_ROW_BYTES]);
            pg.plane2[at..at + PAGE_ROW_BYTES].fill(0xFF);
            pg.compose_row(at / PAGE_ROW_BYTES);
            source += 1;
            at += 80;
            cx -= 1;
        }
        // `@@tc0b`.
        pg.clear(at, PAGE_ROW_BYTES);
    }

    /// `STARS.ASM:679-770`: the two half-blits, into the back page `write`.
    ///
    /// The top 100 rows come from the fetched unit `top`, the bottom 100
    /// from `bottom` with its row order reversed.
    fn blit(&mut self, write: usize, top: u8, bottom: u8) {
        let ring = &self.ring;
        let pg = &mut self.pages[write];
        let top = &ring[top as usize * UNIT_BYTES..][..UNIT_BYTES];
        let bottom = &ring[bottom as usize * UNIT_BYTES..][..UNIT_BYTES];
        for row in 0..100 {
            let src = row * PAGE_ROW_BYTES;
            let dst = BLIT_ROW_STRIDE * row;
            pg.plane0[dst..dst + PAGE_ROW_BYTES]
                .copy_from_slice(&top[src..src + PAGE_ROW_BYTES]);
            pg.plane1[dst..dst + PAGE_ROW_BYTES]
                .copy_from_slice(&top[UNIT_PLANE1 + src..UNIT_PLANE1 + src + PAGE_ROW_BYTES]);
            pg.compose_row(dst / PAGE_ROW_BYTES);

            let src = (99 - row) * PAGE_ROW_BYTES;
            let dst = 8000 + BLIT_ROW_STRIDE * row;
            pg.plane0[dst..dst + PAGE_ROW_BYTES]
                .copy_from_slice(&bottom[src..src + PAGE_ROW_BYTES]);
            pg.plane1[dst..dst + PAGE_ROW_BYTES]
                .copy_from_slice(&bottom[UNIT_PLANE1 + src..UNIT_PLANE1 + src + PAGE_ROW_BYTES]);
            pg.compose_row(dst / PAGE_ROW_BYTES);
        }
    }

    /// One `do_stars` iteration in the source's order; returns the page the
    /// CRTC displays during it.
    fn advance(&mut self, ctx: &mut PartCtx) -> usize {
        let tick = self.tick as usize;
        // The flip: the page written last iteration is the one displayed.
        let displayed = (tick + 1) & 1;
        let write = tick & 1;

        // `:559-648`: the fade, while `starpalfade <= 32`.
        if self.starpalfade <= 32 {
            self.starpalfade += 1;
            let shifted = u16::from(self.starpalfade) * 8;
            let b = if shifted > 255 { 255 } else { shifted as u8 };
            ctx.set_palette(fade_palette(b));
        }

        // `:650-666`.
        self.starframe = self.starframe.wrapping_add(1);
        if self.starframe == MARK_TEXT0 {
            self.startxtp0 = TEXT_BLOCK0;
            self.startxtopen = TEXT_OPEN_START;
            self.startxtclose = TEXT_CLOSE_START;
        }
        if self.starframe == MARK_TEXT1 {
            self.startxtp0 = TEXT_BLOCK1;
            self.startxtopen = TEXT_OPEN_START;
            self.startxtclose = TEXT_CLOSE_START;
        }
        if self.starframe == MARK_STARS {
            self.starlimit = STAR_COUNT as u16;
        }

        // `:667`: ES is still the displayed page's segment here.
        self.risetext(displayed);

        // `:669-678`.
        let unit = self.starfetch0();
        if self.starframe <= STARS_PAUSE_FROM {
            self.star_pass(&[unit], false);
        } else if self.starframe > STARS_PAUSE_TO {
            let second = (unit + UNIT_SECOND as u8) & 63;
            self.star_pass(&[unit, second], true);
        }

        // `:679-770`.
        let bottom = (unit + UNIT_MIRROR as u8) & 63;
        self.blit(write, unit, bottom);

        self.tick += 1;
        displayed
    }

    /// The displayed page as the engine's indexed surface: the page's
    /// composed rows, one 320-pixel copy each.
    fn present(&self, ctx: &mut PartCtx, page: usize) {
        let pg = &self.pages[page];
        let stride = ctx.surface.width();
        let pixels = ctx.surface.pixels_mut();
        for y in 0..SCREEN_HEIGHT {
            let source = y * SCREEN_WIDTH;
            let destination = y * stride;
            pixels[destination..destination + SCREEN_WIDTH]
                .copy_from_slice(&pg.index[source..source + SCREEN_WIDTH]);
        }
    }
}

/// The 0-3 index of every (plane 0 byte, plane 1 byte) pair as eight pixels:
/// bit 7 of each is the leftmost pixel.
fn pair_table() -> &'static [[u8; 8]; 65536] {
    static TABLE: OnceLock<Box<[[u8; 8]; 65536]>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Box::new([[0u8; 8]; 65536]);
        for plane0 in 0..256usize {
            for plane1 in 0..256usize {
                let mut pixels = [0u8; 8];
                for (bit, pixel) in pixels.iter_mut().enumerate() {
                    let shift = 7 - bit;
                    *pixel = (((plane0 >> shift) & 1) | (((plane1 >> shift) & 1) << 1)) as u8;
                }
                table[(plane0 << 8) | plane1] = pixels;
            }
        }
        table
    })
}

impl Part for Ddstars {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `KOE.ASM:74-76` and `STARS.ASM:56-176`: the mode's display state, the
    /// text strip, the ring, the star table and the 100 opening passes.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let image = load_image(NAME, TEXT_PATH)?;
        if (image.width() as usize, image.height() as usize) != (TEXT_WIDTH, TEXT_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{TEXT_PATH} is {}x{}; the text routine reads {TEXT_WIDTH}x{TEXT_HEIGHT}",
                    image.width(),
                    image.height()
                ),
            });
        }
        if image.pixels().iter().any(|pixel| *pixel > 3) {
            return Err(PartError {
                part: NAME,
                message: format!("{TEXT_PATH} is not a two-plane picture"),
            });
        }
        // The decoder's plane path delivers the two 1bpp planes as 2-bit
        // pixels; pack them back to the 40-byte rows `risetext` copies.
        let mut plane0 = vec![0u8; TEXT_WIDTH / 8 * TEXT_HEIGHT];
        let mut plane1 = vec![0u8; TEXT_WIDTH / 8 * TEXT_HEIGHT];
        for row in 0..TEXT_HEIGHT {
            let source = &image.pixels()[row * TEXT_WIDTH..(row + 1) * TEXT_WIDTH];
            for byte in 0..TEXT_WIDTH / 8 {
                let mut a = 0u8;
                let mut b = 0u8;
                for bit in 0..8 {
                    let value = source[byte * 8 + bit];
                    a |= (value & 1) << (7 - bit);
                    b |= ((value >> 1) & 1) << (7 - bit);
                }
                plane0[row * PAGE_ROW_BYTES + byte] = a;
                plane1[row * PAGE_ROW_BYTES + byte] = b;
            }
        }

        // `:80-176`: the pages and `clearsbu`'s ring zeroed once (nothing
        // clears them again), the tables, the state and the opening passes.
        *self = Ddstars::new();
        self.text = [plane0, plane1];
        self.ring = vec![0; RING_UNITS * UNIT_BYTES];
        self.stars = Self::initial_stars(&mut self.rng);
        for _ in 0..INIT_PASSES {
            let unit = self.starfetch0();
            self.star_pass(&[unit], false);
        }
        debug_assert_eq!(self.emmpage4, UNIT_AFTER_INIT);
        debug_assert_eq!(self.starlimit, STAR_ADD as u16 - INIT_PASSES as u16);

        // `resetmode13`'s cleared DAC and the inherited display state.
        ctx.set_palette(Palette::new());
        *ctx.view = mode_view(SCREEN_WIDTH as u16, SCREEN_HEIGHT as u16, None, Some(255));
        // The mode set cleared the picture; the first frame() presents page 0
        // as it stands, which is black until the first pass is blitted.
        let stride = ctx.surface.width();
        let pixels = ctx.surface.pixels_mut();
        for y in 0..SCREEN_HEIGHT {
            pixels[y * stride..y * stride + SCREEN_WIDTH].fill(0);
        }
        Ok(())
    }

    /// One `do_stars` iteration; DIS function 2 is the only way out (`:772-776`).
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.exit_requested {
            self.finished = true;
            return PartStatus::Finished;
        }
        if self.finished {
            return PartStatus::Finished;
        }
        let page = self.advance(ctx);
        self.present(ctx, page);
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::read_asset;
    use crate::parts::test_rig::{reference, rgb_of, Rig};
    use crate::shell::SURFACE_WIDTH;
    use sr_engine::dac_expand;

    /// The reference's mapping, boot = 414 + tick (`docs/fidelity/ddstars.md` §3).
    fn tick_of(index: u32) -> u32 {
        index - 414
    }

    fn started() -> (Rig, Ddstars) {
        let mut rig = Rig::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT as u16);
        let mut part = Ddstars::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    fn run_ticks(rig: &mut Rig, part: &mut Ddstars, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Continuing, "tick {at}");
        }
    }

    fn run_to(rig: &mut Rig, part: &mut Ddstars, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    /// The page tick `tick` displays, and the run's rig.
    fn displayed_at(tick: u32) -> (Rig, Ddstars, usize) {
        let (mut rig, mut part) = started();
        let mut page = 0;
        while part.tick <= tick {
            page = part.advance(&mut rig.ctx(part.tick));
        }
        (rig, part, page)
    }

    #[test]
    fn the_rng_is_the_originals_lcg_and_its_first_values_are_the_sequence() {
        let mut rng = Lcg::new();
        let first: Vec<u16> = (0..12).map(|_| rng.next()).collect();
        assert_eq!(
            first,
            [0, 126, 25208, 45290, 7958, 4852, 38525, 27318, 39895, 10061, 34431, 60585]
        );

        let stars = Ddstars::initial_stars(&mut Lcg::new());
        assert_eq!(stars.len(), STAR_COUNT);
        let head: Vec<(u8, i16, i16)> = stars[..4].iter().map(|s| (s.z, s.x, s.y)).collect();
        assert_eq!(head, [(255, -512, -386), (254, 120, -278), (253, 278, 244), (252, 125, 182)]);
        // `cx` 1024 down to 1 leaves the last entry's z at 0.
        assert_eq!(stars[STAR_COUNT - 1].z, 0);
    }

    #[test]
    fn the_projection_tables_are_the_originals_integer_divisions() {
        let [y, x] = muldiv_tables();
        assert_eq!([y[0], y[1], y[109], y[110], y[179], y[180], y[255]],
            [23592, 22980, 6039, 5998, 4086, 4067, 3024]);
        assert_eq!([x[0], x[110], x[255]], [31457, 7997, 4032]);

        // The three bands (`:317-327`).
        assert_eq!(star_planes(180), (true, false), "z >= 180 is plane 0");
        assert_eq!(star_planes(179), (false, true), "110..179 is plane 1");
        assert_eq!(star_planes(110), (false, true));
        assert_eq!(star_planes(109), (true, true), "z < 110 is both");
    }

    #[test]
    fn the_projection_places_the_star_and_clips_at_the_sources_bounds() {
        // Row 0-99 only, unsigned.
        assert_eq!(project(0, 0, -5), Some((92, 160)));
        assert_eq!(project(0, 0, -1), Some((98, 160)));
        assert_eq!(project(0, 0, -69), Some((0, 160)));
        assert_eq!(project(0, 0, -70), None, "row 100 is outside");
        assert_eq!(project(0, 0, 0), None, "row 100 at y = 0");
        assert_eq!(project(0, 0, 1), None);

        // Column 0-319 only.
        assert_eq!(project(0, -83, -1), Some((98, 0)));
        assert_eq!(project(0, 83, -1), Some((98, 319)));
        assert_eq!(project(0, 84, -1), None, "column 321");
        assert_eq!(project(0, -84, -1), None, "column -2");

        // z = 255 is the smallest muldiv: y = -512 still lands inside.
        assert_eq!(project(255, -512, -512), Some((5, 34)));
        assert_eq!(project(255, 512, -512), Some((5, 286)));
    }

    /// Every entry hidden (`x = y = 512` projects outside, the high `z` keeps
    /// them from respawning) and the gate open.
    fn hide_stars(part: &mut Ddstars) {
        part.stars.fill(Star { z: 250, x: 512, y: 512 });
        part.starlimit = 0;
    }

    /// A fresh ring with one visible star at index 0, z = 100, x = 0, y = -20;
    /// returns its (offset, bit) once one tick has moved it to z = 98.
    fn one_visible_star(part: &mut Ddstars) -> (usize, u8) {
        part.ring.fill(0);
        hide_stars(part);
        part.stars[0] = Star { z: 100, x: 0, y: -20 };
        let (row, column) = project(98, 0, -20).expect("visible");
        (row * PAGE_ROW_BYTES + column / 8, 0x80u8 >> (column & 7))
    }

    #[test]
    fn the_ring_wraps_every_sixty_four_ticks_and_never_clears() {
        let (mut rig, mut part) = started();
        assert_eq!(part.emmpage4, UNIT_AFTER_INIT);
        assert_eq!(part.starlimit, STAR_ADD as u16 - INIT_PASSES as u16);

        let (offset, bit) = one_visible_star(&mut part);
        run_ticks(&mut rig, &mut part, 1);
        // Tick 0 fetched unit 37 and drew z = 98 there, in both planes.
        assert_eq!(part.emmpage4, 37);
        let unit = 37 * UNIT_BYTES;
        assert_ne!(part.ring[unit + offset] & bit, 0, "plane 0");
        assert_ne!(part.ring[unit + UNIT_PLANE1 + offset] & bit, 0, "plane 1");

        // Hide the star and run a whole ring: unit 37 comes round again at
        // tick 64 and still holds the bit.
        hide_stars(&mut part);
        run_ticks(&mut rig, &mut part, 64);
        assert_eq!(part.emmpage4, 37, "the ring wrapped to the same unit");
        assert_ne!(part.ring[unit + offset] & bit, 0, "the unit survived its reuse");
    }

    #[test]
    fn the_second_star_pass_writes_the_same_bits_into_the_plus_seventeen_unit() {
        let (mut rig, mut part) = started();
        let (offset, bit) = one_visible_star(&mut part);
        // The next tick is starframe 1201, staradd2.
        part.starframe = 1200;
        run_ticks(&mut rig, &mut part, 1);
        assert_eq!(part.emmpage4, 37);
        for u in [37, 54] {
            let base = u * UNIT_BYTES;
            assert_ne!(part.ring[base + offset] & bit, 0, "unit {u} plane 0");
            assert_ne!(part.ring[base + UNIT_PLANE1 + offset] & bit, 0, "unit {u} plane 1");
        }
    }

    #[test]
    fn the_blit_reads_the_fetched_unit_on_top_and_the_plus_thirty_two_reversed() {
        // Tick 0 fetches 37; its mirror is (37 + 32) & 63 = 5, rows reversed,
        // so page row 100 reads source row 99.
        let (mut rig, mut part) = started();
        part.ring.fill(0);
        let top = 37usize * UNIT_BYTES;
        let bottom = 5usize * UNIT_BYTES;
        part.ring[top] = 0xAA;
        part.ring[top + UNIT_PLANE1 + 39] = 0x55;
        part.ring[bottom + 99 * PAGE_ROW_BYTES] = 0x11;
        part.ring[bottom + UNIT_PLANE1 + 99 * PAGE_ROW_BYTES + 39] = 0x22;
        hide_stars(&mut part);
        run_ticks(&mut rig, &mut part, 1);
        let page = &part.pages[0];
        assert_eq!(page.plane0[0], 0xAA, "top unit row 0");
        assert_eq!(page.plane1[39], 0x55, "top unit row 0, plane 1");
        assert_eq!(page.plane0[8000], 0x11, "mirror row 99 at page row 100");
        assert_eq!(page.plane1[8000 + 39], 0x22, "mirror row 99 at page row 100, plane 1");
    }

    #[test]
    fn exit_requested_finishes_on_that_tick() {
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 10);
        let at = part.tick;
        assert_eq!(part.frame(&mut rig.ctx_exit(at)), PartStatus::Finished);
        // And the loop stays ended; it does not draw again.
        assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Finished);
        assert_eq!(part.tick, at, "the finished part took no further ticks");
    }

    #[test]
    fn without_the_exit_notice_the_loop_keeps_drawing() {
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 200);
        assert!(!part.finished);
        let drawn = &rig.surface.pixels()[..SURFACE_WIDTH * SCREEN_HEIGHT];
        assert!(drawn.iter().any(|pixel| *pixel != 0), "the field is drawn");
    }

    #[test]
    fn the_palette_fade_is_the_sources_thirty_three_ticks() {
        // One upload a tick while starpalfade <= 32: b = 8 at tick 0, the last
        // two saturate at 255.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 1);
        assert_eq!(part.starpalfade, 1);
        assert_eq!(rig.palette.entry(1), [0, 0, 0], "17*8 >> 8 = 0");
        assert_eq!(rig.palette.entry(5), [10, 20, 35], "the fixed entries");
        run_to(&mut rig, &mut part, 32);
        assert_eq!(part.starpalfade, 32);
        assert_eq!(rig.palette.entry(1), [16, 20, 25], "b = 255");
        assert_eq!(rig.palette.entry(3), [41, 52, 62]);
        run_to(&mut rig, &mut part, 33);
        assert_eq!(part.starpalfade, 33);
        assert_eq!(rig.palette.entry(2), [24, 31, 37]);
        // Then the fade stops and the DAC holds.
        let held = rig.palette.entry(1);
        run_to(&mut rig, &mut part, 40);
        assert_eq!(rig.palette.entry(1), held);
    }

    #[test]
    fn the_text_strip_is_read_as_its_two_planes() {
        let (_, part) = started();
        let raw = read_asset(NAME, TEXT_PATH).unwrap();
        assert_eq!(raw.len(), 16064);
        assert_eq!(&raw[..2], &[0xFC, 0xFC]);
        assert_eq!(part.text[0].len(), TEXT_WIDTH / 8 * TEXT_HEIGHT);
        // The pixel data is at 0x40, 80 bytes a row: the decoder's two planes
        // packed back are the file's own bytes.
        assert_eq!(&part.text[0][..40], &raw[64..104]);
        assert_eq!(&part.text[0][40..80], &raw[64 + 80..64 + 120], "row 1's plane 0");
        assert_eq!(&part.text[1][0..40], &raw[104..144]);
        // The first inked row is 11, the 1900 keyframe's first text line: 12
        // plane-0 bits, 21 pixels with both planes.
        let ink = |row: usize, both: bool| -> u32 {
            (0..40)
                .map(|byte| {
                    let at = row * 40 + byte;
                    let bits = part.text[0][at] | if both { part.text[1][at] } else { 0 };
                    bits.count_ones()
                })
                .sum()
        };
        assert_eq!([ink(10, false), ink(11, false), ink(14, false)], [0, 12, 81]);
        assert_eq!([ink(10, true), ink(11, true)], [0, 21]);
    }

    #[test]
    fn the_full_fade_palette_is_the_references_seven_colours() {
        // `fade_palette(255)` through the 6→8-bit widening.
        let dac = fade_palette(255);
        assert_eq!(dac.entry(1).map(dac_expand), [65, 81, 101]);
        assert_eq!(dac.entry(2).map(dac_expand), [97, 125, 150]);
        assert_eq!(dac.entry(3).map(dac_expand), [166, 210, 251]);
        assert_eq!(dac.entry(5).map(dac_expand), [40, 81, 142]);
        assert_eq!(dac.entry(6).map(dac_expand), [81, 121, 182]);
        assert_eq!(dac.entry(7).map(dac_expand), [121, 162, 243]);
        assert_eq!(dac.entry(4), [0, 0, 0], "entry 4 is black like entry 0");
    }

    #[test]
    fn the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels() {
        // The commented guards (`:692-696`, `:711-716`, `:737-742`,
        // `:756-762`) would skip a blit row whose destination offset is in
        // [`_nostar1`, `_nostar2`]: `_nostar1 = 80*(150-A)` (`:436-442`) and
        // `_nostar2` is 11,879 for every A, set after the closing clear's
        // `add di,80; dec di` (`:525-526`) past the last text row's byte
        // 80*146+40.
        let rows_for = |a: usize| -> (usize, usize, Vec<usize>) {
            let nostar1 = 80 * (150 - a);
            let nostar2 = 80 * 146 + 40 + 80 + 80 - 1;
            // The top blit's offsets are 80r at page row 2r; the bottom's
            // 8000 + 80r at 200 + 2r.
            let mut rows: Vec<usize> = (0..100)
                .flat_map(|r| [(80 * r, 2 * r), (8000 + 80 * r, 200 + 2 * r)])
                .filter(|(offset, _)| (nostar1..=nostar2).contains(offset))
                .map(|(_, row)| row)
                .collect();
            rows.sort_unstable();
            (nostar1, nostar2, rows)
        };
        let (nostar1, nostar2, rows) = rows_for(32);
        assert_eq!((nostar1, nostar2), (9440, 11_879));
        // A = 32 skips only the bottom half: 31 rows, page rows 236-296.
        assert_eq!((rows.len(), rows[0], rows[30]), (31, 236, 296));

        // Frame 1900 holds 1,025 star pixels in exactly those rows, and so
        // does the port's unclipped page; a port with the clip would lose them.
        let stars = [[65u8, 81, 101], [97, 125, 150], [166, 210, 251]];
        let count = |rgb: &[u8]| -> usize {
            rows.iter()
                .flat_map(|y| rgb[y * 320 * 3..(y + 1) * 320 * 3].chunks_exact(3))
                .filter(|pixel| stars.iter().any(|star| star == pixel))
                .count()
        };
        assert_eq!(count(&reference("ddstars", 1900, "rgb")), 1_025, "the reference's");
        let (rig, part, page) = displayed_at(tick_of(1900));
        assert_eq!(count(&rgb_of(&part.pages[page].index, &rig.palette)), 1_025, "the port's");

        // At A = 99 the window reaches into the top half as well.
        let (_, _, rows) = rows_for(99);
        assert_eq!((rows.len(), rows[0], rows[97]), (98, 102, 296));
    }

    #[test]
    fn the_text_and_star_layers_are_on_opposite_row_parities() {
        // At the 1900 keyframe: stars on every even row, the 28-row text band
        // (marked by plane 2) on odd rows 239-293, and nothing else.
        let (_, part, page) = displayed_at(tick_of(1900));
        let pg = &part.pages[page];
        let row = |plane: &[u8], y: usize| {
            plane[y * PAGE_ROW_BYTES..(y + 1) * PAGE_ROW_BYTES].iter().any(|b| *b != 0)
        };
        let stars = |y: usize| row(&pg.plane0, y) || row(&pg.plane1, y);
        let text: Vec<usize> = (0..SCREEN_HEIGHT).filter(|y| row(&pg.plane2, *y)).collect();
        assert!(text.iter().all(|y| y % 2 == 1), "text on an even row: {text:?}");
        assert_eq!((text.len(), text[0], text[27]), (28, 239, 293));
        assert!((0..SCREEN_HEIGHT).step_by(2).all(stars), "an even row without stars");
        let mut odd_without_text = (1..SCREEN_HEIGHT).step_by(2).filter(|y| !text.contains(y));
        assert!(odd_without_text.all(|y| !stars(y)), "plane data on an odd row");
        // The reference's visible ink is on 15 of the band's rows, from 259.
        let ink: Vec<usize> = text.iter().copied().filter(|y| stars(*y)).collect();
        assert_eq!((ink.len(), ink[0]), (15, 259));
    }
}
