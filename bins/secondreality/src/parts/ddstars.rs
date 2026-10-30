//! DDSTARS — the hidden part: a 3-D starfield under a rising text wipe.
//!
//! `DDSTARS/KOE.ASM` (99 lines) is the program and `DDSTARS/STARS.ASM` (803)
//! is the effect, `include`d into it; `POLYEGA.ASM` is linked in but its
//! `poly`/`polyn_disp` routines are never called by the star part. The acts:
//!
//! 1. `KOE.ASM:72-76` — `dis_partstart`, then `resetmode13`: mode 13h, the
//!    attribute controller's entries 0-15 set to their own numbers, overscan
//!    255 and the DAC cleared.
//! 2. `STARS.ASM:56-178` — `init_stars`: mode 13h again, CRTC 9's bit 7
//!    cleared, 64 KB of video cleared, the EMS handle and page frame taken,
//!    `clearsbu` zeroing all 128 units, the row tables, the 1024-star table,
//!    the two `muldiv` tables, `starlimit = 512`, the text state, and **100
//!    `starfetch0`/`staradd` passes** before the first displayed frame.
//! 3. `STARS.ASM:540-778` — `do_stars`, the unbounded loop this part is:
//!    page flip, `waitb`, the palette fade, `starframe`'s stage marks,
//!    `risetext`, one star pass and the two-unit blit. It exits only on DIS
//!    function 2 (`:774-778`), i.e. `exit_requested` in the port.
//!
//! # The screen: 320×400, planar, and two planes of it
//!
//! Mode 13h's CRTC 9 has its scan-doubling bit cleared (`:72-78`) and its
//! offset register is never written, so the screen is 320 pixels by 400 rows
//! with a **40-byte plane row**. The star blits write 40-byte rows at
//! **80-byte steps** (`:697-724`, `:737-770`), so each written row is a page
//! row and the odd rows are untouched padding — which is where the text goes:
//! `risetext` starts its writes 40 bytes into the 80-byte stride (`:450-460`),
//! so the text occupies the *other* half of each row pair. The reference
//! frames show exactly that: star pixels on the even scanlines and text on
//! the odd ones (`reference/parts/ddstars/frame_001900.rgb`; measured in
//! `docs/fidelity/ddstars.md` §3).
//!
//! The port composes the 16-colour index from the page's planes:
//! `index = plane0bit | plane1bit << 1 | plane2bit << 2`. The stars set
//! planes 0 and 1 (`:315-327`, `:378-397`); the text copies write its two
//! source planes into planes 0 and 1 and, through the graphics controller's
//! set/reset (`:479-483`), force **plane 2 = 1** and plane 3 = 0 — so the
//! text's four colours are indices 4-7. That is why the reference has seven
//! distinct colours: entries 1-3 for the stars, 5-7 for the text, and black
//! (entries 0 and 4 both black).
//!
//! # The tick map, the pages, and the one-tick text offset
//!
//! The loop is double-buffered: at the top of an iteration `:544-553` flips
//! the CRTC start between 0 and 0x4000 and points `starvram` at the page it
//! is about to draw. From then on the *displayed* page is the one written
//! last iteration, and — because `risetext` runs before the blit section
//! reloads `es` (`:667` against `:683-684`) — **the text is written into the
//! displayed page, while the stars go into the back page**. The screen at
//! iteration `t` therefore carries the text of `t` and the stars of `t-1`.
//!
//! A tick in the port is one `do_stars` iteration, and the reference's own
//! frames pin the mapping **boot = 414 + tick**: the sweep's 500, 700, 900,
//! …, 8000 and the four filed keyframes are reproduced byte for byte at that
//! offset, with their star rows and their text rows at the same time (the
//! one-tick text offset above is what reconciles them). **This is a mapping,
//! not an anchor.** The reference files no frame at the part's start, and the
//! earliest evidence — a probe at 415 with the screen already up
//! (`reference-indices.md` §22) — is a window. `shell::boot_anchor("ddstars")`
//! stays `None`, a run of this part numbers its own frames from zero, and its
//! manifest says `index_origin: "part"`, `anchor: null`.
//!
//! # The stars, unit for unit
//!
//! 1024 entries of `(z, x, y)`. `z` is read through `movzx` as a byte and
//! decremented by 2 (`:288-290`); a borrow respawns `x` and `y` from the LCG
//! and skips the entry that tick, leaving `z` at its wrapped 254/255. The
//! multiplier is `0x343FD`, the addend `0x269EC3`, the seed starts at **0**,
//! and `random` returns the **`mul`'s `DX`** — the low half of the product's
//! high dword, `(seed * 0x343FD) >> 32` — not the new seed's high word
//! (`:33-42`). The projection uses the two 256-word `muldiv` tables:
//! `row = (y * muldiv_y[z]) >> 14 + 100` and `col = (x * muldiv_x[z]) >> 14 +
//! 160`, where `muldiv_y[z] = (108 << 16) / (150 + 4z) >> 1` and the x table
//! uses 144 (`:141-161`), and an entry draws only when the row is 0-99 and
//! the column 0-319, both compared unsigned. The planes by band are `z >=
//! 180` → plane 0, `110 <= z < 180` → plane 1, `z < 110` → both (`:317-327`).
//!
//! `staradd` walks 512 entries and `staradd2` all 1024 (`:278-411`);
//! `starlimit` gates the first `512 - limit` (or `1024 - limit`) of them and
//! falls by 1 a tick, or by 4 after the mark at `starframe` 1500. From
//! `starframe` 901 through 1200 **nothing is drawn at all** (`:670-676`): the
//! ring keeps being fetched and blitted, so the screen shows the unit's stale
//! contents for 300 frames.
//!
//! # The ring: 64 units of 8 KB, and they are never cleared
//!
//! The original allocates 64 EMS pages and clears them once (`clearsbu`),
//! then **ORs** each star pass into one 8 KB unit per plane pair. Nothing
//! ever clears a unit again, so a unit's picture accumulates every star that
//! has landed in it, and each unit recurs every 64 ticks. The port keeps a
//! plain `Vec<u8>` of [`RING_UNITS`] × [`UNIT_BYTES`]: plane 0 at bytes
//! 0-3999, the 96-byte gap, plane 1 at 4096-8095, exactly the addresses the
//! original's `rows1[40r] + x/8` and `+4096` use. The unit fetched by
//! `starfetch0` is `(previous + 1) & 63`; the top half of the screen reads
//! it, and the bottom half reads `(unit + 32) & 63` **with its rows
//! reversed** (`:726-770`) — so the lower 100 rows are a 32-frame-old mirror
//! of the field.
//!
//! `staradd2` has a second write target: `starfetch1` maps the logical page
//! `(unit + 17) & 63` into EMS physical page 1 and returns a segment there,
//! and the routine writes every star bit to **both** units (`:343-411`).
//! The port keeps that: the same bit is ORed into unit `u` and unit
//! `(u + 17) & 63`. The reference's 1700 frame only reproduces with both
//! (a one-unit model differs in 729 pixels; `docs/fidelity/ddstars.md` §4).
//!
//! # The palette
//!
//! `resetmode13` leaves the DAC black; `do_stars` writes entries 0-7 every
//! tick while `starpalfade <= 32` (`:559-648`): entry 1 is `(17,21,26)·b`,
//! entry 2 `(25,32,38)·b`, entry 3 `(42,53,63)·b`, entry 4 black and
//! entries 5-7 the fixed `(10,20,35)`, `(20,30,45)`, `(30,40,60)`, where
//! `b` is `8·(starpalfade+1)` saturating to 255 on the 8-bit shift's carry.
//! Six bits are kept; the engine widens to eight at upload. The reference's
//! seven colours are those entries through its own rounding
//! ([`sr_engine::dac_expand`]).
//!
//! # D24: the text is the divergence, and this port follows the reference
//!
//! `MAIN/DATA/DDSTARS.EXE` is the shipped, packed build and is not the tree's
//! `KOE.OBJ`; the tree's star-clip tests are assembled out — the
//! `cmp di,_nostar1/_nostar2` guards are commented out at `:692-696`,
//! `:711-716`, `:737-742` and `:756-762`. With them in, the guards would skip
//! every blit row whose 80-byte destination offset lies in
//! `[80*(150-A), 11879]`: at the 1900 tick (A = 32) the bottom half's rows
//! 18-48, page rows 236-296, hold **1,025** star pixels of the reference
//! frame, and at A = 99 the window reaches into both halves. The port keeps
//! the text visible and **does not clip the stars**, and the four filed
//! keyframes — with those stars present in the clip's rows — reproduce with
//! **0 differing bytes**, so the shipped build does not have the clip either.
//! The `A = 2` path is a separate 80-byte `risetext` clear (`:528-537`), not
//! the clip. `docs/fidelity/ddstars.md` §6 records the decision per artifact.
//!
//! # No music calls, and why this part is the one the sync band matters to
//!
//! The part never reads the music: the block's cue is the shell's
//! `MUSIC0` at order 70 (`MAIN/U2.ASM:889-894`), and `SECOND U` is the only
//! way to reach it. Its loop has no cap: the reference is still running at
//! frame 20,000 — so it is the part that spends the most time in
//! `dis_sync`'s upper band: the filed `hiddenu` trace holds `(order << 8) |
//! row > 0x4200` for 81.1 % of its frames (`docs/fidelity/music.md` §11).
//! That band is Phase 3's A15; DDSTARS makes it routine rather than
//! theoretical for any later part that waits on `dis_sync` here. The port's
//! part calls the player zero times.

use std::path::Path;
use std::sync::OnceLock;

use sr_assets::load_path;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "ddstars";

// ------------------------------------------------------------- the screen --

/// 320 pixels a row, 400 rows; mode 13h's CRTC offset is never written, so a
/// plane row is 40 bytes and a page is 400 × 40 bytes a plane.
const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 400;
/// The page row's bytes in one plane: 320 pixels / 8.
const PAGE_ROW_BYTES: usize = 40;
/// One page, one plane: 400 rows × 40 bytes. `display_start` 0 and 0x4000
/// are 16,384 bytes apart in the CPU window, so the two pages do not overlap.
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

/// The reference's mapping: the page the CRTC shows at iteration `t` is the
/// capture at **boot = 414 + t**. **A mapping, not an anchor** — only the
/// tests compare by it; `boot_anchor("ddstars")` stays `None`. Pinned by the
/// filed keyframes and the sweep in `docs/fidelity/ddstars.md` §5.
#[cfg(test)]
const REFERENCE_FIRST_INDEX: u32 = 414;

// ---------------------------------------------------------------- the RNG --

/// `STARS.ASM:33-42`'s `random`, quirks and all.
///
/// ```text
/// eax = 0x343FD; mul seed      ; edx:eax = seed * 0x343FD
/// eax += 0x269EC3; seed = eax  ; the add does not carry into edx
/// ax = dx                      ; the return is the product's high low-half
/// ```
///
/// So the seed is a 32-bit LCG mod 2³² and the returned word is bits 32-47
/// of the **product**, not the new seed's high word. The seed starts at 0
/// (`:32` `seed dd 0`).
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

/// `STARS.ASM:141-151`: the 256-entry y table, built once from `bp = 150 +
/// 4z`: `((108 << 16) / bp) >> 1`.
fn muldiv_y_table() -> &'static [u16; 256] {
    static TABLE: OnceLock<[u16; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u16; 256];
        for (z, entry) in table.iter_mut().enumerate() {
            *entry = (((108u32 << 16) / (150 + 4 * z as u32)) >> 1) as u16;
        }
        table
    })
}

/// `STARS.ASM:152-161`: the x table's numerator is 144.
fn muldiv_x_table() -> &'static [u16; 256] {
    static TABLE: OnceLock<[u16; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = [0u16; 256];
        for (z, entry) in table.iter_mut().enumerate() {
            *entry = (((144u32 << 16) / (150 + 4 * z as u32)) >> 1) as u16;
        }
        table
    })
}

/// One table entry, for the tests and the note's arithmetic.
#[cfg(test)]
fn muldiv_y(z: u8) -> u16 {
    muldiv_y_table()[usize::from(z)]
}

/// One x-table entry.
#[cfg(test)]
fn muldiv_x(z: u8) -> u16 {
    muldiv_x_table()[usize::from(z)]
}

/// `STARS.ASM:294-313` with the tables already in hand.
///
/// The 32-bit signed product is shifted right 14 through `shld dx,ax,2`, then
/// the 16-bit add and the unsigned comparison: `Some((row, column))` when the
/// star is on the 100×320 star bitmap, `None` when the source's `ja @@2`
/// skips it (rows 0-99 and columns 0-319 only).
fn project_with(
    muldiv_y: &[u16; 256],
    muldiv_x: &[u16; 256],
    z: u8,
    x: i16,
    y: i16,
) -> Option<(usize, usize)> {
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

/// [`project_with`] over the static tables.
#[cfg(test)]
fn project(z: u8, x: i16, y: i16) -> Option<(usize, usize)> {
    project_with(muldiv_y_table(), muldiv_x_table(), z, x, y)
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

/// One star table entry: `z`, `x`, `y` in the first three words of the
/// eight-byte record (`STARS.ASM:26` `star dw STARS2 dup(0,0,0,0)`).
///
/// `z` is kept as a byte because the code only ever writes its low byte
/// (`mov ds:[si],bl`, `:290`) and reads it through `movzx`; the high byte the
/// initialiser leaves at zero is never observable.
#[derive(Clone, Copy, Default)]
struct Star {
    z: u8,
    x: i16,
    y: i16,
}

/// One page: the three planes the part writes, and the composed 320×400
/// indices kept in step with them. Plane 3 is never set (the text's
/// set/reset forces it to 0), so it is not stored.
///
/// The index buffer is a cache, not a second source of truth: every write to
/// the planes goes through [`Page::compose_row`] for exactly the rows it
/// touched, and [`Ddstars::present`] is a copy. The unit tests compare the
/// planes to the reference; the integration test compares this composition.
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
    text_path: &'static str,
}

impl Ddstars {
    /// A part with nothing loaded: `init` reads the text strip and builds
    /// the ring, the table and the pages.
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
            text_path: TEXT_PATH,
        }
    }

    /// [`new`](Ddstars::new) reading the text strip from elsewhere: a unit
    /// test's working directory is this crate's, not the repository root.
    #[cfg(test)]
    fn at(path: &'static str) -> Ddstars {
        Ddstars {
            text_path: path,
            ..Ddstars::new()
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
        let muldiv_y = muldiv_y_table();
        let muldiv_x = muldiv_x_table();
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
            let Some((row, column)) = project_with(muldiv_y, muldiv_x, z, star.x, star.y) else {
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

    /// One `do_stars` iteration's state changes; returns the page the CRTC
    /// displays during it.
    ///
    /// The order is the source's: flip, `waitb` (an engine tick), the palette
    /// fade, `starframe` and its marks, `risetext` into the **displayed**
    /// page, the star pass, and the blit into the back page.
    fn advance(&mut self, ctx: &mut PartCtx) -> usize {
        self.advance_with(ctx, true)
    }

    /// [`advance`](Ddstars::advance) with `staradd2`'s second-unit write
    /// switchable: the counterfactual the 1700 survey frame measures.
    fn advance_with(&mut self, ctx: &mut PartCtx, second_unit: bool) -> usize {
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
            if second_unit {
                self.star_pass(&[unit, second], true);
            } else {
                self.star_pass(&[unit], true);
            }
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
        let image = load_path(Path::new(self.text_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.text_path),
        })?;
        if (image.width() as usize, image.height() as usize) != (TEXT_WIDTH, TEXT_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} is {}x{}; the text routine reads {TEXT_WIDTH}x{TEXT_HEIGHT}",
                    self.text_path,
                    image.width(),
                    image.height()
                ),
            });
        }
        if image.pixels().len() != TEXT_WIDTH * TEXT_HEIGHT {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} carries {} pixel bytes; the part reads {}",
                    self.text_path,
                    image.pixels().len(),
                    TEXT_WIDTH * TEXT_HEIGHT
                ),
            });
        }
        if image.pixels().iter().any(|pixel| *pixel > 3) {
            return Err(PartError {
                part: NAME,
                message: format!("{} is not a two-plane picture", self.text_path),
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
        self.text = [plane0, plane1];

        // `:80-98`: both pages are zeroed once, and `clearsbu` clears every
        // ring unit once; after this nothing is ever cleared again.
        for page in &mut self.pages {
            page.plane0.fill(0);
            page.plane1.fill(0);
            page.plane2.fill(0);
            page.index.fill(0);
        }
        self.ring = vec![0; RING_UNITS * UNIT_BYTES];

        // `:102-176`: the tables, the state, and the 100 opening passes.
        self.rng = Lcg::new();
        self.stars = Self::initial_stars(&mut self.rng);
        self.emmpage4 = 0;
        self.starlimit = STAR_ADD as u16;
        self.starpalfade = 0;
        self.starframe = 0;
        self.startxtopen = -9999;
        self.startxtclose = 10000;
        self.startxtp0 = 0;
        self.tick = 0;
        self.finished = false;
        for _ in 0..INIT_PASSES {
            let unit = self.starfetch0();
            self.star_pass(&[unit], false);
        }
        debug_assert_eq!(self.emmpage4, UNIT_AFTER_INIT);
        debug_assert_eq!(self.starlimit, STAR_ADD as u16 - INIT_PASSES as u16);

        // `resetmode13`'s cleared DAC and the inherited display state.
        ctx.set_palette(Palette::new());
        ctx.view.width = SCREEN_WIDTH as u16;
        ctx.view.height = SCREEN_HEIGHT as u16;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = None;
        ctx.view.pan = 0;
        ctx.view.border = Some(255);
        ctx.view.split = None;
        // The mode set cleared the picture; the first frame() presents page 0
        // as it stands, which is black until the first pass is blitted.
        let stride = ctx.surface.width();
        let pixels = ctx.surface.pixels_mut();
        for y in 0..SCREEN_HEIGHT {
            pixels[y * stride..y * stride + SCREEN_WIDTH].fill(0);
        }
        Ok(())
    }

    /// One `do_stars` iteration. The loop is unbounded; the exit flag is the
    /// only way out (`:772-776`), and the port never spins on it.
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
    use crate::parts::test_rig::{differing_bytes, reference, Rig};
    use crate::shell::SURFACE_WIDTH;
    use sr_engine::dac_expand;

    const TEST_TEXT: &str = "../../assets/DDSTARS/TEXTS.16";

    fn started() -> (Rig, Ddstars) {
        let mut rig = Rig::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT as u16);
        let mut part = Ddstars::at(TEST_TEXT);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    fn run_ticks(rig: &mut Rig, part: &mut Ddstars, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            assert_eq!(
                part.frame(&mut rig.ctx(at)),
                PartStatus::Continuing,
                "tick {at}"
            );
        }
    }

    fn run_to(rig: &mut Rig, part: &mut Ddstars, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    // --- the RNG -------------------------------------------------------------

    #[test]
    fn the_rng_is_the_originals_lcg_and_its_first_values_are_the_sequence() {
        // `STARS.ASM:33-42`: EAX = 0x343FD * seed (64-bit), the add to the
        // low half only, and the return is the product's bits 32-47.
        let mut rng = Lcg::new();
        let first: Vec<u16> = (0..12).map(|_| rng.next()).collect();
        assert_eq!(
            first,
            [0, 126, 25208, 45290, 7958, 4852, 38525, 27318, 39895, 10061, 34431, 60585]
        );
        assert_eq!(Lcg::MULTIPLIER, 0x343FD);
        assert_eq!(Lcg::INCREMENT, 0x269EC3);
        // The state wraps at 2^32: the high half of the product is dropped.
        let mut wrapped = Lcg { seed: u32::MAX };
        let _ = wrapped.next();
        assert_eq!(
            wrapped.seed,
            u32::MAX
                .wrapping_mul(Lcg::MULTIPLIER)
                .wrapping_add(Lcg::INCREMENT)
        );

        // The first four table entries, from that stream.
        let mut rng = Lcg::new();
        let stars = Ddstars::initial_stars(&mut rng);
        assert_eq!(stars.len(), STAR_COUNT);
        for (index, (z, x, y)) in [
            (255u8, -512i16, -386i16),
            (254, 120, -278),
            (253, 278, 244),
            (252, 125, 182),
        ]
        .into_iter()
        .enumerate()
        {
            let star = stars[index];
            assert_eq!((star.z, star.x, star.y), (z, x, y), "entry {index}");
        }
        // `cx` 1024 down to 1 leaves the last entry's z at 0.
        assert_eq!(stars[STAR_COUNT - 1].z, 0);
    }

    // --- the projection ------------------------------------------------------

    #[test]
    fn the_projection_tables_are_the_originals_integer_divisions() {
        // `:141-161`: bp = 150 + 4z; y = ((108 << 16) / bp) >> 1, x = 144.
        assert_eq!(muldiv_y(0), 23592);
        assert_eq!(muldiv_y(1), 22980);
        assert_eq!(muldiv_y(109), 6039);
        assert_eq!(muldiv_y(110), 5998);
        assert_eq!(muldiv_y(179), 4086);
        assert_eq!(muldiv_y(180), 4067);
        assert_eq!(muldiv_y(255), 3024);
        assert_eq!(muldiv_x(0), 31457);
        assert_eq!(muldiv_x(110), 7997);
        assert_eq!(muldiv_x(255), 4032);

        // The three bands (`:317-327`).
        assert_eq!(star_planes(180), (true, false), "z >= 180 is plane 0");
        assert_eq!(star_planes(179), (false, true), "110..179 is plane 1");
        assert_eq!(star_planes(110), (false, true));
        assert_eq!(star_planes(109), (true, true), "z < 110 is both");
    }

    #[test]
    fn the_projection_places_the_star_and_clips_at_the_sources_bounds() {
        // `shld dx,ax,2; add dx,100; cmp dx,99; ja`: row 0-99 only, unsigned.
        assert_eq!(project(0, 0, -5), Some((92, 160)));
        assert_eq!(project(0, 0, -1), Some((98, 160)));
        assert_eq!(project(0, 0, -69), Some((0, 160)));
        assert_eq!(project(0, 0, -70), None, "row 100 is outside");
        assert_eq!(project(0, 0, 0), None, "row 100 at y = 0");
        assert_eq!(project(0, 0, 1), None);

        // `add dx,160; cmp dx,319; ja`: column 0-319 only.
        assert_eq!(project(0, -83, -1), Some((98, 0)));
        assert_eq!(project(0, 83, -1), Some((98, 319)));
        assert_eq!(project(0, 84, -1), None, "column 321");
        assert_eq!(project(0, -84, -1), None, "column -2");

        // A far star: z = 255 is the smallest muldiv, so y = -512 still lands
        // inside the bitmap and x = -512 is column 34.
        assert_eq!(project(255, -512, -512), Some((5, 34)));
        assert_eq!(project(255, 512, -512), Some((5, 286)));
    }

    // --- the ring ------------------------------------------------------------

    /// One visible star, every other entry hidden: `x = y = 512` projects
    /// outside both bounds, and the high `z` keeps them from respawning
    /// during the test. The visible star is the caller's, at index 0.
    fn one_visible_star(part: &mut Ddstars, star: Star) {
        part.stars.fill(Star {
            z: 250,
            x: 512,
            y: 512,
        });
        part.stars[0] = star;
        part.starlimit = 0;
    }

    #[test]
    fn the_ring_wraps_every_sixty_four_ticks_and_never_clears() {
        let (mut rig, mut part) = started();
        // `init_stars` leaves emmpage4 at 100 & 63 and starlimit at
        // 512 - 100.
        assert_eq!(part.emmpage4, UNIT_AFTER_INIT);
        assert_eq!(part.starlimit, STAR_ADD as u16 - INIT_PASSES as u16);
        assert_eq!(part.ring.len(), RING_UNITS * UNIT_BYTES);

        // A fresh ring, one star at z = 100, x = 0, y = -20.
        part.ring.fill(0);
        one_visible_star(
            &mut part,
            Star {
                z: 100,
                x: 0,
                y: -20,
            },
        );
        run_ticks(&mut rig, &mut part, 1);
        // Tick 0 fetched unit 37 and drew z = 98 there; z < 110 is both
        // planes.
        assert_eq!(part.emmpage4, 37);
        let (row, column) = project(98, 0, -20).expect("visible");
        let offset = row * PAGE_ROW_BYTES + column / 8;
        let bit = 0x80u8 >> (column & 7);
        let unit = 37 * UNIT_BYTES;
        assert_ne!(part.ring[unit + offset] & bit, 0, "plane 0");
        assert_ne!(part.ring[unit + UNIT_PLANE1 + offset] & bit, 0, "plane 1");

        // Hide the star and run a whole ring: unit 37 is fetched again at
        // tick 64, and the bit drawn at tick 0 is still there — nothing
        // clears a unit.
        for other in &mut part.stars {
            other.x = 512;
            other.y = 512;
        }
        run_ticks(&mut rig, &mut part, 64);
        assert_eq!(part.emmpage4, 37, "the ring wrapped to the same unit");
        assert_ne!(
            part.ring[unit + offset] & bit,
            0,
            "the unit survived its reuse"
        );

        // And the wrap is exactly 64: the unit fetched at tick 64 is the one
        // fetched at tick 0.
        assert_eq!(37u8, (UNIT_AFTER_INIT + 1) & 63);
        assert_eq!((37u8 + 64) & 63, 37);
    }

    #[test]
    fn the_second_star_pass_writes_the_same_bits_into_the_plus_seventeen_unit() {
        let (mut rig, mut part) = started();
        part.ring.fill(0);
        one_visible_star(
            &mut part,
            Star {
                z: 100,
                x: 0,
                y: -20,
            },
        );
        // starframe 1200 -> the next tick is 1201, staradd2.
        part.starframe = 1200;
        run_ticks(&mut rig, &mut part, 1);
        assert_eq!(part.starframe, 1201);
        let unit = part.emmpage4;
        let second = (unit + UNIT_SECOND as u8) & 63;
        assert_eq!(unit, 37);
        assert_eq!(second, 54);
        let (row, column) = project(98, 0, -20).expect("visible");
        let offset = row * PAGE_ROW_BYTES + column / 8;
        let bit = 0x80u8 >> (column & 7);
        for u in [unit, second] {
            let base = u as usize * UNIT_BYTES;
            assert_ne!(
                part.ring[base + offset] & bit,
                0,
                "unit {u} plane 0: the same pass writes both"
            );
            assert_ne!(part.ring[base + UNIT_PLANE1 + offset] & bit, 0, "unit {u} plane 1");
        }
    }

    #[test]
    fn the_blit_reads_the_fetched_unit_on_top_and_the_plus_thirty_two_reversed() {
        // `:679-770`: the top half is unit u rows 0-99; the bottom is unit
        // (u + 32) & 63 with the rows reversed, so page row 100 reads source
        // row 99.
        let (mut rig, mut part) = started();
        part.ring.fill(0);
        part.emmpage4 = UNIT_AFTER_INIT;
        // Tick 0 fetches 37; its mirror is (37 + 32) & 63 = 5.
        let top = 37usize * UNIT_BYTES;
        let bottom = 5usize * UNIT_BYTES;
        part.ring[top] = 0xAA;
        part.ring[top + UNIT_PLANE1 + 39] = 0x55;
        part.ring[bottom + 99 * PAGE_ROW_BYTES] = 0x11;
        part.ring[bottom + UNIT_PLANE1 + 99 * PAGE_ROW_BYTES + 39] = 0x22;
        part.stars.fill(Star {
            z: 250,
            x: 512,
            y: 512,
        });
        part.starlimit = 0;
        run_ticks(&mut rig, &mut part, 1);
        let page = &part.pages[0];
        assert_eq!(page.plane0[0], 0xAA, "top unit row 0");
        assert_eq!(page.plane1[39], 0x55, "top unit row 0, plane 1");
        assert_eq!(page.plane0[8000], 0x11, "mirror row 99 at page row 100");
        assert_eq!(
            page.plane1[8000 + 39],
            0x22,
            "mirror row 99 at page row 100, plane 1"
        );
    }

    // --- the exit ------------------------------------------------------------

    #[test]
    fn exit_requested_finishes_on_that_tick() {
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 10);
        let at = part.tick;
        assert_eq!(part.frame(&mut rig.ctx_exit(at)), PartStatus::Finished);
        assert!(part.finished);
        // And the loop stays ended; it does not spin or draw again.
        assert_eq!(part.frame(&mut rig.ctx(at)), PartStatus::Finished);
        assert_eq!(part.tick, at, "the finished part took no further ticks");
    }

    #[test]
    fn without_the_exit_notice_the_loop_keeps_drawing() {
        // `do_stars` has no frame cap: 200 ticks all continue, and the
        // picture is not blank.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 200);
        assert!(!part.finished);
        let drawn = rig.surface.pixels()[..SURFACE_WIDTH * SCREEN_HEIGHT].to_vec();
        assert!(drawn.iter().any(|pixel| *pixel != 0), "the field is drawn");
    }

    #[test]
    fn the_palette_fade_is_the_sources_thirty_three_ticks() {
        // `:559-648`: while starpalfade <= 32, one upload a tick. At tick 0
        // b = 8; the last two uploads saturate at 255.
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
        // The last upload, and then the fade stops; the DAC holds.
        assert_eq!(rig.palette.entry(2), [24, 31, 37]);
        let held = rig.palette.entry(1);
        run_to(&mut rig, &mut part, 40);
        assert_eq!(rig.palette.entry(1), held);
    }

    #[test]
    fn the_text_strip_is_read_as_its_two_planes() {
        let (_, part) = started();
        let raw = std::fs::read(TEST_TEXT).unwrap();
        assert_eq!(raw.len(), 16064);
        assert_eq!(&raw[..2], &[0xFC, 0xFC]);
        assert_eq!(part.text[0].len(), TEXT_WIDTH / 8 * TEXT_HEIGHT);
        // `_textpic`'s pixel data is at 0x40 and each row is 80 bytes: the
        // decoder's two planes packed back must be the file's own bytes.
        assert_eq!(&part.text[0][..40], &raw[64..104]);
        assert_eq!(
            &part.text[0][40..80],
            &raw[64 + 80..64 + 120],
            "row 1's plane 0 is 80 bytes into the source"
        );
        assert_eq!(&part.text[1][0..40], &raw[104..144]);
        // The source's first pixel row with any ink is row 11 — the row the
        // 1900 keyframe's first text line shows. Its plane-0 bits are 12.
        let ink = |row: usize| {
            (0..40)
                .map(|byte| part.text[0][row * 40 + byte].count_ones())
                .sum::<u32>()
        };
        assert_eq!(ink(10), 0);
        assert_eq!(ink(11), 12);
        assert_eq!(ink(14), 81);
        // The first row with ink has 21 pixels once both planes are combined
        // — the reference's 1900 frame counts 21 text pixels on scanline 259.
        let combined = |row: usize| {
            (0..40)
                .map(|byte| (part.text[0][row * 40 + byte] | part.text[1][row * 40 + byte]).count_ones())
                .sum::<u32>()
        };
        assert_eq!(combined(10), 0);
        assert_eq!(combined(11), 21);
    }

    #[test]
    fn the_full_fade_palette_is_the_references_seven_colours() {
        // `fade_palette(255)` through the engine's 6→8-bit widening: the six
        // non-black colours the reference's keyframes carry.
        let dac = fade_palette(255);
        assert_eq!(dac.entry(1).map(dac_expand), [65, 81, 101]);
        assert_eq!(dac.entry(2).map(dac_expand), [97, 125, 150]);
        assert_eq!(dac.entry(3).map(dac_expand), [166, 210, 251]);
        assert_eq!(dac.entry(5).map(dac_expand), [40, 81, 142]);
        assert_eq!(dac.entry(6).map(dac_expand), [81, 121, 182]);
        assert_eq!(dac.entry(7).map(dac_expand), [121, 162, 243]);
        assert_eq!(dac.entry(4), [0, 0, 0], "entry 4 is black like entry 0");
    }

    // --- the reference's mapping and its four keyframes ----------------------

    /// The tick whose displayed page the reference frame at `index` holds:
    /// **boot = 414 + tick**. A mapping, not an anchor.
    fn tick_of(index: u32) -> u32 {
        index - REFERENCE_FIRST_INDEX
    }

    /// The displayed page composed exactly as [`Ddstars::present`] composes
    /// it, through the live palette — the RGB a dump writes.
    fn page_rgb(part: &Ddstars, page: usize, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(SCREEN_WIDTH * SCREEN_HEIGHT * 3);
        for y in 0..SCREEN_HEIGHT {
            let offset = y * PAGE_ROW_BYTES;
            for byte in 0..PAGE_ROW_BYTES {
                let plane0 = part.pages[page].plane0[offset + byte];
                let plane1 = part.pages[page].plane1[offset + byte];
                let plane2 = part.pages[page].plane2[offset + byte];
                for bit in 0..8 {
                    let shift = 7 - bit;
                    let index = usize::from(
                        ((plane0 >> shift) & 1)
                            | (((plane1 >> shift) & 1) << 1)
                            | (((plane2 >> shift) & 1) << 2),
                    );
                    let entry = palette.entry(index as u8);
                    out.push(dac_expand(entry[0]));
                    out.push(dac_expand(entry[1]));
                    out.push(dac_expand(entry[2]));
                }
            }
        }
        out
    }

    #[test]
    #[ignore = "measurement: reads the survey capture out/b5-su-sweep1; run with --ignored"]
    fn the_plus_seventeen_write_is_what_the_survey_frame_shows() {
        // `reference-indices.md` §22.1's sweep holds boot 1700. At the
        // mapping that is tick 1286, and the survey frame is reproduced only
        // when `staradd2`'s second unit is written: the one-unit model
        // differs in 729 pixels, i.e. 2,187 RGB bytes.
        //
        // The capture is not in `reference/` (it is the survey's own
        // `out/b5-su-sweep1`, and `out/` is gitignored); the command that
        // makes it is §22.4's capture with `--frames 1700`.
        // The sweep's other frames pin the rest of the mapping: 500 at the
        // 100-pass field; 900 still before the star pause; 1100 also before
        // it (tick 686, starframe 687, inside `staradd`); 1500 inside it
        // (tick 1086, starframe 1087); 2100 after the mark at 1500 lets the
        // field fill in four entries a tick; and the late ones.
        let sweep = [500u32, 700, 900, 1100, 1300, 1500, 1700, 2100, 3000, 5000, 8000];
        let tick_of = |boot: u32| boot - REFERENCE_FIRST_INDEX;
        let last = tick_of(*sweep.last().unwrap());
        let (mut rig, mut part) = started();
        let mut captured = std::collections::BTreeMap::new();
        while part.tick <= last {
            let tick = part.tick;
            let page = part.advance_with(&mut rig.ctx(tick), true);
            if sweep.iter().any(|boot| tick_of(*boot) == tick) {
                captured.insert(tick, page_rgb(&part, page, &rig.palette));
            }
        }
        for boot in sweep {
            let theirs =
                std::fs::read(format!("../../out/b5-su-sweep1/frame_{boot:06}.rgb")).unwrap();
            assert_eq!(theirs.len(), 320 * 400 * 3);
            assert_eq!(
                differing_bytes(&captured[&tick_of(boot)], &theirs),
                0,
                "boot {boot} (tick {})",
                tick_of(boot)
            );
        }

        // The 1700 counterfactual: the one-unit model.
        let theirs =
            std::fs::read("../../out/b5-su-sweep1/frame_001700.rgb").unwrap();
        let run = |second_unit: bool| -> Vec<u8> {
            let (mut rig, mut part) = started();
            let mut page = 0;
            // Boot 1700 is the mapping's tick 1286; the page that tick
            // presents is the one written at 1285 plus tick 1286's text.
            while part.tick <= 1286 {
                let tick = part.tick;
                page = part.advance_with(&mut rig.ctx(tick), second_unit);
            }
            page_rgb(&part, page, &rig.palette)
        };
        let with = run(true);
        assert_eq!(differing_bytes(&with, &theirs), 0, "the source's two writes");
        let without = run(false);
        assert_eq!(
            differing_bytes(&without, &theirs),
            729 * 3,
            "the one-unit model: 729 pixels, every channel apart"
        );
    }

    #[test]
    fn the_mapping_is_boot_equals_four_hundred_fourteen_plus_the_tick() {
        // The four filed keyframes, by the mapping.
        assert_eq!(tick_of(1900), 1486);
        assert_eq!(tick_of(3000), 2586);
        assert_eq!(tick_of(5000), 4586);
        assert_eq!(tick_of(8000), 7586);
        // The reference's earliest probe has the screen up at 415, which is
        // this mapping's first displayed field (tick 1); tick 0 presents the
        // never-written page. The part's start remains a window, so this is
        // a mapping and `boot_anchor("ddstars")` stays `None`.
        assert_eq!(REFERENCE_FIRST_INDEX + 1, 415);
        assert_eq!(crate::shell::boot_anchor("ddstars"), None);
    }

    #[test]
    fn the_four_keyframes_match_the_reference_at_the_mapping() {
        // Every keyframe is moving (`reference-indices.md` §22.3), so D19's
        // support is a frame on either side, and the four are compared byte
        // for byte against the reference's RGB.
        let keyframes = [1900u32, 3000, 5000, 8000];
        let mut wanted = Vec::new();
        for index in keyframes {
            let tick = tick_of(index);
            wanted.extend([tick - 1, tick, tick + 1]);
        }
        let (mut rig, mut part) = started();
        let mut captured: std::collections::BTreeMap<u32, Vec<u8>> = Default::default();
        let last = *wanted.iter().max().unwrap();
        while part.tick <= last {
            let tick = part.tick;
            let page = part.advance(&mut rig.ctx(tick));
            if wanted.contains(&tick) {
                captured.insert(tick, page_rgb(&part, page, &rig.palette));
            }
        }

        for index in keyframes {
            let tick = tick_of(index);
            let mine = &captured[&tick];
            let theirs = reference("ddstars", index, "rgb");
            assert_eq!(
                differing_bytes(mine, &theirs),
                0,
                "boot {index} (tick {tick})"
            );

            // The reference's own description, computed here so the note can
            // cite the numbers: the distinct colours and the mean byte.
            let mut colours = std::collections::BTreeSet::new();
            for pixel in theirs.chunks_exact(3) {
                colours.insert([pixel[0], pixel[1], pixel[2]]);
            }
            let mean = theirs.iter().map(|byte| u64::from(*byte)).sum::<u64>() as f64
                / theirs.len() as f64;
            let (expected_colours, expected_mean) = match index {
                1900 => (7, 7.6920),
                3000 => (7, 24.8134),
                5000 => (7, 42.5059),
                _ => (4, 57.2442),
            };
            assert_eq!(colours.len(), expected_colours, "boot {index}");
            assert!(
                (mean - expected_mean).abs() < 0.0001,
                "boot {index}: mean {mean}"
            );

            // D19: the neighbours are different pictures, which is what
            // makes the mapping exact rather than a search's nearest fit.
            for neighbour in [tick - 1, tick + 1] {
                assert_ne!(
                    captured[&neighbour], *mine,
                    "boot {index}: tick {neighbour} is not distinguishable"
                );
            }
        }
    }

    #[test]
    fn the_star_clip_is_assembled_out_and_the_keyframes_hold_its_pixels() {
        // The commented guards (`:692-696`, `:711-716`, `:737-742`,
        // `:756-762`) skip a blit row whose 80-byte destination offset is in
        // [`_nostar1`, `_nostar2`]. `_nostar1 = 80*(150-A)` (`:436-442`);
        // the last text row is page row 293 (byte 80*146+40) for every A,
        // and `_nostar2`, set after the closing clear's `add di,80; dec di`
        // (`:525-526`), is 11,879.
        let rows_for = |a: usize| -> (usize, usize, Vec<usize>) {
            let nostar1 = 80 * (150 - a);
            let nostar2 = 80 * 146 + 40 + 80 + 80 - 1;
            // The top blit's offsets are 80r; the bottom's 8000 + 80r.
            let rows = (0..100)
                .map(|r| (80 * r, 2 * r, 8000 + 80 * r, 200 + 2 * r))
                .flat_map(|(top, top_row, bottom, bottom_row)| {
                    let mut rows = Vec::new();
                    if (nostar1..=nostar2).contains(&top) {
                        rows.push(top_row);
                    }
                    if (nostar1..=nostar2).contains(&bottom) {
                        rows.push(bottom_row);
                    }
                    rows
                })
                .collect::<Vec<usize>>();
            let mut rows = rows;
            rows.sort_unstable();
            (nostar1, nostar2, rows)
        };
        let (nostar1, nostar2, rows) = rows_for(32);
        assert_eq!((nostar1, nostar2), (9440, 11_879));
        // A = 32 skips only the bottom half: 31 rows, page rows 236-296.
        assert_eq!(rows.len(), 31);
        assert_eq!((rows[0], rows[30]), (236, 296));

        // Frame 1900 holds 1,025 star pixels in exactly those rows; the
        // port's unclipped page holds the same, and the keyframe test shows
        // the whole frame is byte-identical. A port with the clip would
        // lose them.
        let stars = [(65u8, 81, 101), (97, 125, 150), (166, 210, 251)];
        let theirs = reference("ddstars", 1900, "rgb");
        let count = |rgb: &[u8]| -> usize {
            rows.iter()
                .map(|y| {
                    (0..320)
                        .filter(|x| {
                            let at = (y * 320 + x) * 3;
                            stars.contains(&(rgb[at], rgb[at + 1], rgb[at + 2]))
                        })
                        .count()
                })
                .sum()
        };
        assert_eq!(count(&theirs), 1_025, "the reference's clip-row stars");

        let (mut rig, mut part) = started();
        let mut page = 0;
        while part.tick <= tick_of(1900) {
            page = part.advance(&mut rig.ctx(part.tick));
        }
        let mine = page_rgb(&part, page, &rig.palette);
        assert_eq!(count(&mine), 1_025, "the port's unclipped page");

        // At A = 99 the window reaches into the top half as well.
        let (_, _, rows) = rows_for(99);
        assert_eq!(rows.len(), 98);
        assert_eq!((rows[0], rows[97]), (102, 296));
    }

    #[test]
    fn the_text_and_star_layers_are_on_opposite_row_parities() {
        // The blits write 40 bytes every 80, so stars land on even rows; the
        // text's first write is 40 bytes into the stride, so it lands on odd
        // rows. At the 1900 keyframe both layers are present and they do not
        // share a row — which is why the keyframe is a clean composition of
        // the two.
        let (mut rig, mut part) = started();
        let tick = tick_of(1900);
        let mut page = 0;
        while part.tick <= tick {
            page = part.advance(&mut rig.ctx(part.tick));
        }
        let mut star_rows = 0;
        let mut text_rows = 0;
        let mut first_text = None;
        let mut last_text = None;
        for y in 0..SCREEN_HEIGHT {
            let offset = y * PAGE_ROW_BYTES;
            let planes = part.pages[page].plane0[offset..offset + PAGE_ROW_BYTES]
                .iter()
                .chain(&part.pages[page].plane1[offset..offset + PAGE_ROW_BYTES])
                .any(|byte| *byte != 0);
            let text = part.pages[page].plane2[offset..offset + PAGE_ROW_BYTES]
                .iter()
                .any(|byte| *byte != 0);
            if text {
                // The text's set/reset marks its rows; they are the odd ones.
                assert_eq!(y % 2, 1, "row {y} has text on an even row");
                text_rows += 1;
                first_text.get_or_insert(y);
                last_text = Some(y);
            }
            if y % 2 == 0 {
                assert!(planes, "even row {y} is a star row and has no bits");
                star_rows += 1;
            } else if !text {
                assert!(!planes, "odd row {y} has plane data but no text");
            }
        }
        assert_eq!(star_rows, 200, "the two 100-row halves");
        assert_eq!(
            text_rows, 28,
            "the band the source clears/copies is 28 rows; plane 2 marks all of them"
        );
        // The reference's visible ink is on 15 of those rows, starting at
        // scanline 259.
        let ink_rows: Vec<usize> = (239..=293)
            .step_by(2)
            .filter(|y| {
                let offset = y * PAGE_ROW_BYTES;
                part.pages[page].plane0[offset..offset + PAGE_ROW_BYTES]
                    .iter()
                    .chain(&part.pages[page].plane1[offset..offset + PAGE_ROW_BYTES])
                    .any(|byte| *byte != 0)
            })
            .collect();
        assert_eq!((ink_rows.len(), ink_rows.first()), (15, Some(&259)));
        // The band is bottom-anchored: A = 32 puts its top at 239 and its
        // bottom at 293 for every open height.
        assert_eq!((first_text, last_text), (Some(239), Some(293)));
    }
}
