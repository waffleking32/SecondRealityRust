//! PLZPART — the plasma and the cube.
//!
//! `PLZPART/PLZ.C` (219 lines) is the plasma, `PLZPART/VECT.C` (299) the cube,
//! `PLZPART/PLZFILL.C` (186) the cube's scene, palette and polygon setup,
//! `PLZPART/PLZA.ASM` (375) the span rasteriser and the clear, `PLZPART/COPPER.ASM`
//! (300) the three copper routines, `PLZPART/ASMYT.ASM` (119) and
//! `PLZPART/SPLINE.ASM` (88) the plasma line and the camera spline, and
//! `PLZPART/TWEAK.ASM` (350) the two mode sets. `PLZPART/MAIN.C` runs
//! `init_copper(); initvect(); plz(); vect(); close_copper()`: **one part, two
//! screens**.
//!
//! # The data
//!
//! The part has no media file: its tables are the original's `.INC` text
//! fragments, copied verbatim to `assets/PLZPART/` and parsed here in their own
//! `db`/`dw` format. `ASMYT.ASM:12-24` includes `PSINI.INC` (16,384 `db`
//! bytes), `LSINI4.INC` and `LSINI16.INC` (8,192 `dw` words each);
//! `INCLUDE.ASM:33-36` includes `SINIT.INC` (the 2,048-byte sine table and a
//! 527-byte repeat) as the word array `sinit[1024]`, with `kosinit=sinit+512`;
//! `SPLINE.ASM:15-21` includes `RATA.INC` (136 control points, 1,088 `dw`
//! words — 36 written lines plus `REPT 100`)
//! and `SPLINE.INC` (1,024 `dw` coefficients); `PLZ.C:41-42` includes
//! `PTAU.INC` (129 `db` values).
//!
//! **The four displacements in `ASMYT.ASM:46-61` are placeholders** — `0xC100`,
//! `0xC200`, `0xC300` and `0xC400` sit inside the `plzline` instructions and
//! `setplzparas` (`ASMYT.ASM:74-95`) overwrites each one with a table address. The map
//! gives the table bases: `_psini` at `027D:0000`, `_lsini4` at `027D:4000`,
//! `_lsini16` at `027D:8000` and `_plzline` at `027D:C000`
//! (`PLZPART.MAP:368-369,391`), so the placeholders are in `plzline`'s own code
//! and not table addresses at all. The index arithmetic `setplzparas` patches
//! in is what [`plasma_line`] implements:
//!
//! ```text
//! byte r of line y (r = 0..83), parameters (c1,c2,c3,c4):
//!   b  = lsini16[c2 + 320 - 4r + y]
//!   px = psini[c1 + 8r + b]
//!   d  = lsini4[c4 + 16r + y]
//!   py = psini[c3 + 320 - 4r + d + 2y]
//!   value = (px + py) as u8          ; an 8-bit add, wrapping
//! ```
//!
//! # The plasma
//!
//! `plz()` (`PLZ.C:62-122`): wait for `musplus() >= 0`, `setmframe(0)`,
//! `init_plz` (`:124-218`), then the frame loop. The mode set is
//! `tw_opengraph2` (`TWEAK.ASM:44-67`): chain-4 off, CRTC 0x14 = 0, 0x17 =
//! `0xe3` (byte mode), 0x09 = 0 (400 rows), 0x13 = `0x3013` (a **96-byte plane
//! row**), and the 64 KB clear. `cop_start = 96*(682-400) = 27,072` plane bytes
//! (`PLZ.C:176`) and `set_plzstart(60)` sets the line compare to 60. The
//! display is 320×400 with the upper view starting past the drawn plasma
//! (rows 0-60 are video memory at 27,072, black) and, from the split down, the
//! lower view reading plane byte 0 — the 280 drawn rows. `Split::scanline` is
//! the register **plus one** (61/62), which Phase 1 measured on the reference
//! (`reference-indices.md` §13.2).
//!
//! `pompota` (`COPPER.ASM:126-143`) alternates the line compare 60/61 and the
//! attribute pan register 4/0. The port's `View.pan` is in **pixels** and a
//! 256-colour mode shifts `(register & 7) / 2` (`view.rs`, CRED's measurement),
//! so the pan is 2/0; the split is 61/62.
//!
//! # The cube
//!
//! `vect()` (`VECT.C:110-130`): `tw_opengraph()` (`TWEAK.ASM:18-37`) — chain-4
//! off, 0x09 = 2, 0x13 = `0x5013` (a **160-byte plane row**), 200 rows — then
//! wait for `musplus() >= 13`, then per frame `swappage`, the frame wait,
//! `calculate(1)`, `draw(15)`, `clear()`. The six-page flip (`VECT.C:265-298`)
//! alternates three 0x5500-byte buffers at 0, 0x5500 and 0xAA00 and moves the
//! display start to the buffer drawn a frame earlier.
//!
//! **The cube's beam is composed by the part.** The VGA's four planes are
//! 4 × 65,536 = 262,144 bytes of linear memory (`vga_memory.cpp`'s unchecked
//! planar writes mask at `vmemwrap>>2 = 0xFFFF`), and the display's linear
//! address wraps at 262,144 (`linear_mask = vmemwrap - 1`). The engine's
//! surface is 640×400 = 256,000 pixels — 6,144 short — so a `View` over it
//! would wrap 1,536 plane bytes early, and the cube's bottom rows (whose
//! display start 0xAA28 reaches past 65,536 during three of the six pages)
//! would read the wrong bytes. The part therefore keeps the four-plane image in
//! its own 262,144-byte array and writes the visible 320×200 window into the
//! surface with the engine's `View` as a plain 320×200 window.
//!
//! # The copper
//!
//! `init_copper` (`COPPER.ASM:44-65`) registers `copper1` on the DIS's slot 1,
//! `copper2` on slot 2 and the empty `copper3` on slot 0; the DIS's numbering is
//! `0 = after display start, 1 = just before retrace, 2 = in the retrace`
//! (`DIS/DISINT.ASM:284-291`) and the engine's order is 0 → bands → 1 → 2
//! (`engine.rs`). The mapping is therefore the identity, and it is stated by
//! what each routine does:
//!
//! ```text
//! engine slot 0  <- copper3  empty
//! engine slot 1  <- copper1  the CRTC start/pan writer
//! engine slot 2  <- copper2  frame_count++, the palette upload,
//!                            pompota + moveplz, do_drop
//! ```
//!
//! One tick is one copper frame. The engine composes a tick's bands **before**
//! slot 2 runs, so the state slot 2 leaves is seen from the next tick on; the
//! original's retrace is the same boundary. `pompi` therefore starts at 1 (the
//! source's BSS 0 plus the one retrace that lands before the first composed
//! frame), which is what makes the measured 60→61 alternation line up with the
//! reference's frames (test `the_split_and_pan_alternate_at_the_reference_parity`).
//!
//! # The preserved bugs
//!
//! - **The 84-byte `plzline` overrun.** `ASMYT.ASM:41-69` writes 21 dwords = 84
//!   bytes per row while the visible row is 80 plane bytes (320 pixels), so the
//!   last 4 bytes land in the 96-byte row's stride gap. Invisible, and kept.
//! - **`do_clear`'s zero-count mask underflow.** `PLZA.ASM:350-363` indexes
//!   `end_mask` with `x-1`; at `x = 0` that is the byte *before* the table,
//!   which is `start_mask`'s last entry `0x0F` — the same value the periodic
//!   table would give. The port reads the same flat table.
//! - **The palette-leak banding.** `fadepal` is 1,536 bytes: 768 high (colour)
//!   and 768 low (fraction) bytes (`COPPER.ASM:184,238-252`).
//!   `memset(fadepal,0,768)` (`PLZ.C:81`) clears the colour half and leaves the
//!   fraction standing, so the next table's fade starts from the previous
//!   table's fractional state. Both halves are modelled as 768 `u16`
//!   accumulators, and the reset keeps the low byte.
//! - **The cut-short last fade, and the exit.** The exit is
//!   `curpal == EXIT_PALETTE && cop_drop > 64`; the drop's last step resets the
//!   line compare and never accumulates, so the last table's fade is cut
//!   short. The tree's `EXIT_PALETTE` is 5 (`PLZ.C:94`, `PLZ.OBJ`'s
//!   `cmpw $5`), which would end the plasma four switches in, at boot 8769;
//!   the **release ends it after three switches** — its 320×400 screen is gone
//!   by boot 8340 (`reference-indices.md` §13.7) and the reference's 8500 frame
//!   is the cube. The port adopts the release's 4 (`EXIT_PALETTE_RELEASE`) as
//!   D24's single-value rule requires; `APPROXIMATIONS.md` A22 records the
//!   measurement.
//! - **The 404 split value.** `do_drop`'s `dtau` table reaches 404
//!   (`COPPER.ASM:175-181`), so the drop walks the split off the bottom of the
//!   screen; the port writes 405 as `Split::scanline`, which is inert.
//! - **The double `+160`.** `xxx = (xx*256)/zz + 160 + 160` (`VECT.C:198`).
//! - **The dead `ttptr` row.** `timetable[4] = 2778` (`PLZ.C:46`) can never be
//!   reached: the exit fires 65 ticks after the previous switch.
//! - **The C precedence in `count_const`.** `>>15+7` is `>> (15+7)` = `>>22`
//!   (`VECT.C:166-176`; `VECT.OBJ` disassembles to `sar $0x16`), and the
//!   `cyx`/`cyy`/`czx`/`czy` lines subtract before the final shift. The port
//!   keeps both.
//!
//! The stale `PLZPART/MAIN.ASM` is not part of the port; `MAIN.C` is the part's
//! entry point.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::{Split, View};
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "plzpart";

// ---------------------------------------------------------------- the data --

/// `ASMYT.ASM:12-24`: `psini` is `db 16384 dup` — 16,384 bytes.
const PSINI_LEN: usize = 16_384;
/// `ASMYT.ASM:18-24`: `lsini4` is `dw 8192 dup` — 8,192 words.
const LSINI_WORDS: usize = 8_192;
/// `INCLUDE.ASM:33-36` + `SINIT.INC`: the file's 2,575 bytes; the C reads the
/// first 1,024 words as `sinit` and `kosinit = sinit + 512`.
const SINIT_BYTES: usize = 2_575;
/// `PLZ.C:41-42`: `char ptau[256]`, of which `PTAU.INC` initialises 129.
const PTAU_LEN: usize = 129;
/// `SPLINE.ASM:19-20`: 1,024 `dw` coefficients.
const SPLINE_WORDS: usize = 1_024;
/// `SPLINE.ASM:15-16`: `buu`, the camera control points, 136 × 8 words.
const RATA_WORDS: usize = 1_088;
/// How many words of the sine segment the cube can read: `kosinit[k]` is
/// `sinit[k + 512]` for `k` up to 1,023, so 1,536 words at most.
const SINIT_WORDS: usize = 1_536;

/// The table bases, from `PLZPART.MAP`: `_psini 027D:0000`, `_lsini4
/// 027D:4000`, `_lsini16 027D:8000`, `_plzline 027D:C000`.
const MAP_PSINI: u16 = 0x0000;
const MAP_LSINI4: u16 = 0x4000;
const MAP_LSINI16: u16 = 0x8000;
#[cfg(test)]
const MAP_PLZLINE: u16 = 0xC000;
/// The placeholders the assembly carries before `setplzparas` patches them
/// (`PLZA.ASM:46-61`). They are inside `plzline`'s instructions and are not
/// table addresses — the map's `_plzline` is at 0xC000.
#[cfg(test)]
const PLACEHOLDERS: [u16; 4] = [0xC100, 0xC200, 0xC300, 0xC400];

// ------------------------------------------------------------- the plasma --

/// `TWEAK.ASM:59-66`: CRTC 0x13 = 0x30 → a 96-byte plane row.
const PLASMA_ROW_BYTES: usize = 96;
/// The visible row: 320 pixels = 80 plane bytes.
/// `PLZ.C:176`: `cop_start = 96*(682-400)` plane bytes.
const PLASMA_COP_START: u32 = 96 * (682 - 400);
/// The engine's pixel stride for a 96-byte plane row: four pixels a byte.
const PLASMA_STRIDE: u16 = (PLASMA_ROW_BYTES * 4) as u16;
/// `PLZ.C:11`: `MAXY 280` — the plasma's drawn rows.
const PLASMA_ROWS: usize = 280;
/// `PLZ.C:10`: `LINELEN 41` — unused by the assembly, kept for the source's
/// shape.
#[allow(dead_code)]
const PLASMA_LINELEN: usize = 41;
/// The screen is 320×400 (`reference-indices.md` §13).
const PLASMA_WIDTH: u16 = 320;
const PLASMA_HEIGHT: u16 = 400;
/// `PLZ.C:46`: the timetable, in `getmframe` ticks. The last non-zero entry is
/// dead (the exit fires first); the trailing 0 is the array's terminator.
const TIMETABLE: [i32; 6] = [723, 1491, 1875, 2259, 2778, 0];
/// The tree's `PLZ.C:94`: `curpal == 5` (confirmed in `PLZ.OBJ`: `cmpw
/// $0x5,0x100`). The release exits one table earlier; see the module docs.
#[cfg(test)]
const EXIT_PALETTE_TREE: i32 = 5;
/// The release's exit index, measured: after the **third** switch the exit fires
/// 65 ticks later, and the reference's 320×400 screen is gone by boot 8340
/// (`reference-indices.md` §13.7). Adopted under D24; A22 records it.
const EXIT_PALETTE_RELEASE: i32 = 4;
/// `PLZ.C:46`'s `inittable` — the eight parameters each switch restores.
const INITTABLE: [[i32; 8]; 6] = [
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 4000, 4000, 1500, 2300, 3900, 1670],
    [3500, 1000, 3000, 1000, 3500, 3300, 2900, 2670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
];

// --------------------------------------------------------------- the cube --

/// `TWEAK.ASM:34-35`: CRTC 0x13 = 0x50 → a 160-byte plane row.
const CUBE_ROW_BYTES: usize = 160;
const CUBE_WIDTH: u16 = 320;
const CUBE_HEIGHT: u16 = 200;
/// The four-plane linear image: 4 bytes per plane byte, 65,536 addresses.
const VIDEO_BYTES: usize = 4 * 65_536;
/// vga_draw.cpp's `linear_mask = vmemwrap - 1`.
const VIDEO_MASK: u32 = (VIDEO_BYTES - 1) as u32;
/// `VECT.C:267-297`: the three buffers are 0x5500 bytes apart.
/// `VECT.C:270-296`: the six display starts, in page order.
const PAGE_START: [u16; 6] = [0xAA00 + 40, 40, 0x5500 + 40, 0xAA00 + 40, 40, 0x5500 + 40];
/// `VECT.C:270-296`: the six attribute-pan values, in page order.
const PAGE_PAN: [u8; 6] = [4, 0, 4, 0, 4, 0];
/// The three draw buffers, in page order.
const PAGE_VMEM: [u16; 6] = [0x0000, 0x5500, 0xAA00, 0x0000, 0x5500, 0xAA00];
/// `PLZFILL.C:22,67`: the clear's eight rotation slots.
const CLR_SLOTS: usize = 8;
/// `PLZA.ASM:77`: `cmp yy,134` — the cube's drawn rows.
const CUBE_DRAWN_ROWS: i32 = 134;
/// `PLZA.ASM:305`: the clear walks 134+1 lines; the mask loop 134.
const CLEAR_LINES: i32 = 135;

/// `ASMYT.ASM`'s two 200-entry periodic mask tables: `start_mask = 1111b,
/// 1110b, 1100b, 1000b` and `end_mask = 0001b, 0011b, 0111b, 1111b`
/// (`PLZA.ASM:13-20`), flattened so `do_clear`'s `end_mask[x-1]` at x = 0 reads
/// the byte before `end_mask` — `start_mask`'s last entry — exactly as the
/// original does.
const MASK_TABLE_LEN: usize = 1_600;
const START_MASK_AT: usize = 0;
const END_MASK_AT: usize = 800;

/// `VECT.C:101-106`: the object's points and faces.
const CUBE_POINTS: [[i32; 3]; 8] = [
    [125, 125, 125],
    [125, -125, 125],
    [-125, -125, 125],
    [-125, 125, 125],
    [125, 125, -125],
    [125, -125, -125],
    [-125, -125, -125],
    [-125, 125, -125],
];
const CUBE_FACES: [[i32; 8]; 6] = [
    [1, 2, 3, 0, 0, 0, 0, 0],
    [7, 6, 5, 4, 0, 0, 0, 0],
    [0, 4, 5, 1, 0, 0, 0, 1],
    [1, 5, 6, 2, 0, 0, 0, 2],
    [2, 6, 7, 3, 0, 0, 0, 1],
    [3, 7, 4, 0, 0, 0, 0, 2],
];
/// `PLZFILL.C:88`: the texture rectangle.
const TEXTURE_POINTS: [[i32; 2]; 4] = [[64, 4], [190, 4], [190, 60], [64, 60]];

// ------------------------------------------------------------- the music --

/// The reference's part tick 0 is boot **5723**: `_np_zframe` reads 0 there
/// and 5730 is 320×400, so the setmframe(0) at `PLZ.C:69` lands at 5723
/// (`music-clock.md` §2's table: "from frame 5723 ... the part's
/// `_dis_setmframe(0)`"). A mapping for the comparison, not a boot anchor.
#[cfg(test)]
const REFERENCE_FIRST_INDEX: u32 = 5723;
/// `moveplz` walks the parameters **before** the plasma's mode set, while the
/// part waits for the music: `MAIN.C` registers the copper before `plz()`, and
/// `COP_PLZ` starts at 1 (`COPPER.ASM:19`), so every retrace during the wait
/// walks `k`/`l`. The reference's 6000 frame fixes the number: its `plzline`
/// values at boot 6000 are the initial parameters walked **345** times, and the
/// mode set is at boot 5723 (part tick 0), so the wait was **68** ticks
/// (`the_release_wait_walks_the_plasma_parameters_sixty_eight_times`). The
/// waiting ticks themselves are not part of the port's numbering: the shell
/// seeks the player to the mode-set position, and the pre-walk stands in for
/// the ticks the reference spent before it.
const PLASMA_WAIT_WALKS: u32 = 68;
/// The entry run's head start, in polls: the shell seeks MUSIC1 to order 62
/// (`music_for_part`), and the player's published words are eleven polls behind
/// the reference's when the part starts. Nine pre-polls put the reference's
/// `musplus() >= 13` (its cube gate) on boot 8364 (part tick 2641), which is
/// the trace's first order-68 row 13. Measured by
/// `the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`, guarded on
/// `ctx.music_fresh()` like the Phase-4 parts' pre-rolls.
const MUSIC_PRE_ROLL: u32 = 9;
/// `VECT.C:116`: `while(dis_musplus()<13 ...)`.
const CUBE_GATE: i32 = 13;
/// `VECT.C:119-120`: break when `-4 <= musplus < 0`.
const CUBE_EXIT_LOW: i32 = -4;

// -------------------------------------------------------------- the tables --

/// The seven `.INC` fragments, parsed.
struct Tables {
    psini: Vec<u8>,
    lsini4: Vec<i16>,
    lsini16: Vec<i16>,
    sinit: Vec<i16>,
    ptau: Vec<i16>,
    spline: Vec<i16>,
    rata: Vec<i16>,
    /// The `poro` segment as the assembly addresses it: `psini` at 0x0000,
    /// `lsini4` at 0x4000 and `lsini16` at 0x8000 (`PLZPART.MAP`). A read
    /// below `lsini16` — `setplzparas` can place the lc2 displacement at
    /// `0x7FD8` — lands in `lsini4`'s last bytes, exactly as the original's
    /// 16-bit address does.
    segment: Vec<u8>,
}

/// Evaluate one `.INC` token: a decimal, a symbol, or `a*b` where either side
/// is a decimal or a symbol.
fn eval_token(token: &str, symbols: &HashMap<String, i32>) -> Result<i32, String> {
    let token = token.trim();
    if token.is_empty() {
        return Err("empty token".into());
    }
    let look = |name: &str| -> Result<i32, String> {
        symbols
            .get(name)
            .copied()
            .or_else(|| name.parse::<i32>().ok())
            .ok_or_else(|| format!("unknown symbol {name:?}"))
    };
    if let Some((left, right)) = token.split_once('*') {
        return Ok(look(left.trim())?.wrapping_mul(look(right.trim())?));
    }
    look(token)
}

/// Parse one of the original's `.INC` fragments.
///
/// A line whose first token is `db`/`dw` carries values; `name=number` lines
/// define the assembler symbols the data lines use (`RATA.INC`'s `kkk`);
/// `;` starts a comment, and `PTAU.INC` has no keyword at all (its values sit
/// between braces). Values are returned in file order.
fn parse_inc(text: &str) -> Result<Vec<i32>, String> {
    let mut symbols: HashMap<String, i32> = HashMap::new();
    let mut values = Vec::new();
    let mut saw_keyword = false;
    // `REPT n` … `ENDM` (RATA.INC's 100 repeated control points). The block's
    // values are emitted once by the lines themselves and then repeated n-1
    // times here.
    let mut repeats: Vec<(usize, usize)> = Vec::new();
    for (line_number, raw) in text.lines().enumerate() {
        let line = raw.split(';').next().unwrap_or("");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper.starts_with("REPT") {
            let count = trimmed
                .split_whitespace()
                .last()
                .and_then(|token| token.parse::<usize>().ok())
                .ok_or_else(|| format!("line {}: REPT needs a count", line_number + 1))?;
            repeats.push((count, values.len()));
            continue;
        }
        if upper.starts_with("ENDM") {
            let (count, start) = repeats
                .pop()
                .ok_or_else(|| format!("line {}: ENDM without REPT", line_number + 1))?;
            let block = values[start..].to_vec();
            for _ in 1..count {
                values.extend_from_slice(&block);
            }
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            let name = name.trim();
            let value = value.trim();
            if !name.is_empty() && !name.contains(['\t', ' ']) {
                if let Ok(parsed) = value.parse::<i32>() {
                    symbols.insert(name.to_string(), parsed);
                    continue;
                }
            }
        }
        let starts = trimmed.starts_with("db") || trimmed.starts_with("dw");
        if starts {
            saw_keyword = true;
        } else if saw_keyword {
            // Continuation lines do not exist in these files; anything else is
            // a comment or a label.
            continue;
        }
        let body = if starts { &trimmed[2..] } else { trimmed };
        for token in body.split(|c: char| c == ',' || c == '{' || c == '}' || c.is_whitespace()) {
            if token.trim().is_empty() {
                continue;
            }
            values.push(
                eval_token(token, &symbols)
                    .map_err(|error| format!("line {}: {error}", line_number + 1))?,
            );
        }
    }
    Ok(values)
}

impl Tables {
    fn load_at(directory: &Path) -> Result<Tables, PartError> {
        let read = |name: &str| -> Result<String, PartError> {
            let path = directory.join(name);
            std::fs::read_to_string(&path).map_err(|error| PartError {
                part: NAME,
                message: format!("{}: {error}", path.display()),
            })
        };
        let parse = |name: &str, text: &str| -> Result<Vec<i32>, PartError> {
            parse_inc(text).map_err(|error| PartError {
                part: NAME,
                message: format!("{name}: {error}"),
            })
        };
        let psini_values = parse("PSINI.INC", &read("PSINI.INC")?)?;
        let lsini4_values = parse("LSINI4.INC", &read("LSINI4.INC")?)?;
        let lsini16_values = parse("LSINI16.INC", &read("LSINI16.INC")?)?;
        let sinit_values = parse("SINIT.INC", &read("SINIT.INC")?)?;
        let ptau_values = parse("PTAU.INC", &read("PTAU.INC")?)?;
        let spline_values = parse("SPLINE.INC", &read("SPLINE.INC")?)?;
        let rata_values = parse("RATA.INC", &read("RATA.INC")?)?;
        let tables = Tables {
            psini: psini_values.iter().map(|&v| v as u8).collect(),
            lsini4: lsini4_values.iter().map(|&v| v as i16).collect(),
            lsini16: lsini16_values.iter().map(|&v| v as i16).collect(),
            sinit: Vec::new(),
            ptau: ptau_values.iter().map(|&v| v as i16).collect(),
            spline: spline_values.iter().map(|&v| v as i16).collect(),
            rata: rata_values.iter().map(|&v| v as i16).collect(),
            segment: Vec::new(),
        };
        tables.check_sizes()?;
        let mut tables = tables;
        tables.sinit = tables.build_sinit(&sinit_values);
        tables.segment = tables.build_segment();
        Ok(tables)
    }

    fn check_sizes(&self) -> Result<(), PartError> {
        let checks = [
            ("PSINI.INC", self.psini.len(), PSINI_LEN),
            ("LSINI4.INC", self.lsini4.len(), LSINI_WORDS),
            ("LSINI16.INC", self.lsini16.len(), LSINI_WORDS),
            ("PTAU.INC", self.ptau.len(), PTAU_LEN),
            ("SPLINE.INC", self.spline.len(), SPLINE_WORDS),
            ("RATA.INC", self.rata.len(), RATA_WORDS),
        ];
        for (name, found, wanted) in checks {
            if found != wanted {
                return Err(PartError {
                    part: NAME,
                    message: format!(
                        "{name} holds {found} values; the original's array is {wanted}"
                    ),
                });
            }
        }
        Ok(())
    }

    /// The `poro` segment's first 0xC000 bytes: the three tables in the map's
    /// order, so a 16-bit address that runs off one table's end reads the
    /// next one's bytes.
    fn build_segment(&self) -> Vec<u8> {
        let mut segment = vec![0u8; 0xC000];
        for (index, &value) in self.psini.iter().enumerate() {
            segment[MAP_PSINI as usize + index] = value;
        }
        for (table, base) in [(&self.lsini4, MAP_LSINI4), (&self.lsini16, MAP_LSINI16)] {
            for (index, &value) in table.iter().enumerate() {
                let at = base as usize + index * 2;
                segment[at..at + 2].copy_from_slice(&value.to_le_bytes());
            }
        }
        segment
    }

    /// `plzline`'s four lookups, at 16-bit segment addresses.
    fn plasma_value(&self, r: i32, y: i32, c1: i32, c2: i32, c3: i32, c4: i32) -> u8 {
        let word_at = |address: u16| -> i32 {
            let at = address as usize;
            i16::from_le_bytes([self.segment[at], self.segment[at + 1]]) as i32
        };
        let byte_at = |address: u16| -> u8 { self.segment[address as usize] };
        // `mov bx,[ds:si+disp]`: lsini16 - 8r + 640 + 2c2 + 2y.
        let b = word_at(
            MAP_LSINI16
                .wrapping_add(2 * c2 as u16)
                .wrapping_sub(8 * r as u16)
                .wrapping_add(640)
                .wrapping_add(2 * y as u16),
        );
        // lc1: psini + c1 + 8r + bx.
        let px = byte_at(
            MAP_PSINI
                .wrapping_add(c1 as u16)
                .wrapping_add(8 * r as u16)
                .wrapping_add(b as u16),
        );
        // lc4: lsini4 + 32r + 2c4 + 2y.
        let d = word_at(
            MAP_LSINI4
                .wrapping_add(32 * r as u16)
                .wrapping_add(2 * c4 as u16)
                .wrapping_add(2 * y as u16),
        );
        // lc3: psini + 320 - 4r + c3 + bx + 2y.
        let py = byte_at(
            MAP_PSINI
                .wrapping_add(320)
                .wrapping_sub(4 * r as u16)
                .wrapping_add(c3 as u16)
                .wrapping_add(d as u16)
                .wrapping_add(2 * y as u16),
        );
        px.wrapping_add(py)
    }

    /// The sine segment as the C reads it.
    ///
    /// `sinit[0..1024]` is the file's first 2,048 bytes. The file's remaining
    /// 527 bytes are the stray repeat `INCLUDE.ASM:33-36` also links
    /// (`KAKKA5` is 0xA0F bytes), and `kosinit[k] = sinit[k+512]` can reach
    /// `sinit[1535]`. Past byte 2,575 the linker puts the next paragraph's data:
    /// one padding byte, then `PLZFILL5_DATA`'s `vmem` (a far pointer
    /// `MK_FP(0xa000,0)`) and `buu`, the RATA words (`PLZPART.MAP:25-26`,
    /// `:270`). The port keeps that continuation byte for byte; it is only read
    /// for `k > 775`, and the keyframes' `k` stay below it.
    fn build_sinit(&self, values: &[i32]) -> Vec<i16> {
        let mut bytes = Vec::with_capacity(SINIT_WORDS * 2);
        bytes.extend(values.iter().map(|&v| v as u8));
        debug_assert_eq!(bytes.len(), SINIT_BYTES);
        // The padding byte at 2D84F, then `vmem = MK_FP(0xa000,0)`:
        // offset 0x0000, segment 0xa000.
        bytes.extend_from_slice(&[0x00, 0x00, 0x00, 0x00, 0xA0]);
        for &value in &self.rata {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.resize(SINIT_WORDS * 2, 0);
        bytes
            .chunks_exact(2)
            .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
            .collect()
    }
}

// ------------------------------------------------------- the plasma line --

/// `plzline` (`ASMYT.ASM:26-72`): 84 bytes for row `y`, four bytes a dword,
/// with the four parameter words `c1..c4`.
///
/// The byte for row index `r` is the 8-bit sum of the two `psini` lookups
/// (module docs). The assembly writes 21 dwords — the last four bytes past the
/// 320-pixel row, into the 96-byte stride's gap — and the port keeps all 84.
fn plasma_line(tables: &Tables, y: usize, c1: i32, c2: i32, c3: i32, c4: i32) -> [u8; 84] {
    let mut out = [0u8; 84];
    for (r, value) in out.iter_mut().enumerate() {
        *value = tables.plasma_value(r as i32, y as i32, c1, c2, c3, c4);
    }
    out
}

// ---------------------------------------------------------- the palette --

/// The pad's 6-bit colour from one uploaded byte: the DAC's low six bits.
fn dac6(value: u8) -> u8 {
    value & 0x3F
}

/// Build `pals[6][768]` exactly as `init_plz` does (`PLZ.C:181-217`).
fn build_pals(tables: &Tables) -> [[i16; 768]; 6] {
    let ptau = &tables.ptau;
    let mut pals = [[0i16; 768]; 6];
    // --- pals[0], "RGB": entry 1 onward; entry 0 stays zero. ---
    let mut at = 3usize;
    for a in 1..64 {
        pals[0][at] = ptau[a];
        pals[0][at + 1] = ptau[0];
        pals[0][at + 2] = ptau[0];
        at += 3;
    }
    for a in 0..64 {
        pals[0][at] = ptau[63 - a];
        pals[0][at + 1] = ptau[0];
        pals[0][at + 2] = ptau[0];
        at += 3;
    }
    for a in 0..64 {
        pals[0][at] = ptau[0];
        pals[0][at + 1] = ptau[0];
        pals[0][at + 2] = ptau[a];
        at += 3;
    }
    for a in 0..64 {
        pals[0][at] = ptau[a];
        pals[0][at + 1] = ptau[0];
        pals[0][at + 2] = ptau[63 - a];
        at += 3;
    }
    // --- pals[1], "RB-black". ---
    let mut at = 3usize;
    for a in 1..64 {
        pals[1][at] = ptau[a];
        pals[1][at + 1] = ptau[0];
        pals[1][at + 2] = ptau[0];
        at += 3;
    }
    for a in 0..64 {
        pals[1][at] = ptau[63 - a];
        pals[1][at + 1] = ptau[0];
        pals[1][at + 2] = ptau[a];
        at += 3;
    }
    for a in 0..64 {
        pals[1][at] = ptau[0];
        pals[1][at + 1] = ptau[a];
        pals[1][at + 2] = ptau[63 - a];
        at += 3;
    }
    for a in 0..64 {
        pals[1][at] = ptau[a];
        pals[1][at + 1] = ptau[63];
        pals[1][at + 2] = ptau[a];
        at += 3;
    }
    // --- pals[3], "RB-white". ---
    let mut at = 3usize;
    for a in 1..64 {
        pals[3][at] = ptau[a];
        pals[3][at + 1] = ptau[0];
        pals[3][at + 2] = ptau[0];
        at += 3;
    }
    for a in 0..64 {
        pals[3][at] = ptau[63];
        pals[3][at + 1] = ptau[a];
        pals[3][at + 2] = ptau[a];
        at += 3;
    }
    for a in 0..64 {
        pals[3][at] = ptau[63 - a];
        pals[3][at + 1] = ptau[63 - a];
        pals[3][at + 2] = ptau[63];
        at += 3;
    }
    for _ in 0..64 {
        pals[3][at] = ptau[0];
        pals[3][at + 1] = ptau[0];
        pals[3][at + 2] = ptau[63];
        at += 3;
    }
    // --- pals[2], "white". ---
    let mut at = 3usize;
    for _ in 1..64 {
        pals[2][at] = ptau[0] / 2;
        pals[2][at + 1] = ptau[0] / 2;
        pals[2][at + 2] = ptau[0] / 2;
        at += 3;
    }
    for &value in ptau.iter().take(64) {
        pals[2][at] = value / 2;
        pals[2][at + 1] = value / 2;
        pals[2][at + 2] = value / 2;
        at += 3;
    }
    for a in 0..64 {
        pals[2][at] = ptau[63 - a] / 2;
        pals[2][at + 1] = ptau[63 - a] / 2;
        pals[2][at + 2] = ptau[63 - a] / 2;
        at += 3;
    }
    for _ in 0..64 {
        pals[2][at] = ptau[0] / 2;
        pals[2][at + 1] = ptau[0] / 2;
        pals[2][at + 2] = ptau[0] / 2;
        at += 3;
    }
    // --- pals[4], "white II". ---
    let mut at = 3usize;
    for a in 1..75 {
        let v = ptau[(63 - a * 64 / 75) as usize];
        pals[4][at] = v;
        pals[4][at + 1] = v;
        pals[4][at + 2] = v;
        at += 3;
    }
    for _ in 0..106 {
        pals[4][at] = 0;
        pals[4][at + 1] = 0;
        pals[4][at + 2] = 0;
        at += 3;
    }
    for a in 0..75 {
        let v = ptau[(a * 64 / 75) as usize];
        pals[4][at] = v * 8 / 10;
        pals[4][at + 1] = v * 9 / 10;
        pals[4][at + 2] = v;
        at += 3;
    }
    // `PLZ.C:215-217`: `(v-63)*2` over `pals[0]`, then `*8` over
    // `pals[1..4]`. `pals[5]` is never built.
    for value in pals[0].iter_mut() {
        *value = (*value - 63) * 2;
    }
    for value in pals[1..5].iter_mut().flat_map(|table| table.iter_mut()) {
        *value = value.wrapping_mul(8);
    }
    pals
}

/// `do_drop`'s `dtau` table (`COPPER.ASM:175-181`): 65 words,
/// `c*c/4*43/128 + 60`.
fn build_dtau() -> [u16; 65] {
    let mut table = [0u16; 65];
    for (c, value) in table.iter_mut().enumerate() {
        let c = c as u32;
        *value = (c * c / 4 * 43 / 128 + 60) as u16;
    }
    table
}

// ------------------------------------------------------------------ the part --

/// Where the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// The plasma's `while(dis_musplus()<0)` gate (`PLZ.C:68`). Fresh runs pass
    /// it at tick 0: the shell's seek to order 62 already stands where the
    /// reference's gate stood (`music_for_part`), and the `254` at order 61
    /// that gave the reference `zplus == 2` is in the past, so an entry run's
    /// gate is the measured position, not a `musplus` reading.
    PlasmaGate,
    /// `PLZ.C:76-118`: the plasma's frame loop.
    Plasma,
    /// `PLZ.C:119`: `while(frame_count==0);` — one tick.
    PlasmaEndWait,
    /// `VECT.C:114-116`: the mode set, then the cube's music gate.
    CubeGate,
    /// `VECT.C:117-128`: `calculate(1); draw(15); clear();` a tick.
    Cube { swapped: bool },
    /// `VECT.C:119-120`: the cube's exit.
    Done,
}

/// PLZPART: the plasma and the cube.
pub struct Plzpart {
    tables: Option<Tables>,
    /// Directory the `.INC` fragments are read from.
    table_directory: &'static str,
    /// Everything the three copper routines own, shared with their closures.
    copper: Rc<RefCell<CopperState>>,
    /// The part's phase.
    phase: Phase,
    /// The part's own tick.
    tick: u32,
    /// Whether this run's player was installed for this part (`music_fresh`).
    fresh: bool,
    /// The pre-roll a fresh run makes before its first tick.
    music_pre_roll: u32,

    // ---- the cube ----
    /// The sine segment as the C reads it (`sinit[1536]`).
    sinit: Vec<i16>,
    /// The four-plane linear image.
    video: Vec<u8>,
    /// `PLZA.ASM:13-20`: the two periodic plane masks, flattened.
    masks: Vec<u8>,
    /// `PLZFILL.C:12`: the runtime sine table, 2,000 `int`s.
    sini: Vec<i16>,
    /// `PLZFILL.C:21`: the cube's own palette, 768 bytes.
    pal: [u8; 768],
    kuva1: Vec<u8>,
    kuva2: Vec<u8>,
    kuva3: Vec<u8>,
    dist1: Vec<u8>,
    /// `PLZFILL.C:22,24`: the eight edge-list slots and the rotation cursor.
    clrtau: Vec<[i32; 2]>,
    clrptr: usize,
    /// `VECT.C:101-108`: the object, its projected points and the draw list.
    points: [[i32; 3]; 8],
    xx: [[i32; 3]; 8],
    xxx: [[i32; 2]; 8],
    pg: [[i32; 8]; 6],
    ptodraw: [i32; 256],
    polys: i32,
    light_src: [i32; 6],
    lls: [i32; 6],
    cxx: i32,
    cxy: i32,
    cxz: i32,
    cyx: i32,
    cyy: i32,
    cyz: i32,
    czx: i32,
    czy: i32,
    czz: i32,
    kx: i32,
    ky: i32,
    kz: i32,
    dis: i32,
    tx: i32,
    ty: i32,
    ls_kx: i32,
    ls_ky: i32,
    ls_x: i32,
    ls_y: i32,
    ls_z: i32,
    page: usize,
    frames: i32,
    // The rasteriser's live span state (`PLZA.ASM`'s globals).
    ryy: i32,
    xx1: i32,
    xx2: i32,
    txx1: i32,
    txy1: i32,
    txx2: i32,
    txy2: i32,
    ax1: i32,
    ax2: i32,
    tax1: i32,
    tay1: i32,
    tax2: i32,
    tay2: i32,
    to_row: i32,
    from_texture: usize,
    dseg_row: usize,
    ctau_row: i32,
}

/// Where the copper's pending palette upload reads from (`cop_pal`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PalSource {
    Fade,
    Cube,
}

impl Plzpart {
    /// A part with nothing loaded: `init` reads the seven fragments.
    pub fn new() -> Plzpart {
        Plzpart {
            tables: None,
            table_directory: "assets/PLZPART",
            copper: Rc::new(RefCell::new(CopperState::new())),
            phase: Phase::PlasmaGate,
            tick: 0,
            fresh: false,
            music_pre_roll: MUSIC_PRE_ROLL,
            sinit: Vec::new(),
            video: Vec::new(),
            masks: build_masks(),
            sini: Vec::new(),
            pal: [0; 768],
            kuva1: Vec::new(),
            kuva2: Vec::new(),
            kuva3: Vec::new(),
            dist1: Vec::new(),
            clrtau: vec![[640, 0]; CLR_SLOTS * 256],
            clrptr: 0,
            points: CUBE_POINTS,
            xx: [[0; 3]; 8],
            xxx: [[0; 2]; 8],
            pg: CUBE_FACES,
            ptodraw: [0; 256],
            polys: 0,
            light_src: [0; 6],
            lls: [0; 6],
            cxx: 0,
            cxy: 0,
            cxz: 0,
            cyx: 0,
            cyy: 0,
            cyz: 0,
            czx: 0,
            czy: 0,
            czz: 0,
            kx: 0,
            ky: 0,
            kz: 0,
            dis: 320,
            tx: 0,
            ty: -50,
            ls_kx: 0,
            ls_ky: 0,
            ls_x: 0,
            ls_y: 0,
            ls_z: 0,
            page: 0,
            frames: 0,
            ryy: 0,
            xx1: 0,
            xx2: 0,
            txx1: 0,
            txy1: 0,
            txx2: 0,
            txy2: 0,
            ax1: 0,
            ax2: 0,
            tax1: 0,
            tay1: 0,
            tax2: 0,
            tay2: 0,
            to_row: 0,
            from_texture: 0,
            dseg_row: 0,
            ctau_row: 0,
        }
    }
}

// ------------------------------------------------------------------------
// The copper: `COPPER.ASM`, shared between the part's frame loop and the
// three callbacks the engine runs.
// ------------------------------------------------------------------------

/// Everything the three copper routines own.
///
/// `PLZ.C` reads `cop_drop`, `curpal` and the parameter words from the same
/// globals `COPPER.ASM` writes, so the part and the callbacks share one state.
struct CopperState {
    /// `PLZ.C:44`: `pals[6][768]`.
    pals: [[i16; 768]; 6],
    /// `COPPER.ASM:175-181`: the drop's line-compare table.
    dtau: [u16; 65],
    /// `COPPER.ASM:184`: 768 16-bit accumulators, colour in the high byte.
    fadepal: [u16; 768],
    /// The table the fade accumulates (`cop_fadepal`).
    fade_index: usize,
    /// `PLZ.C:45`: the next `pals` table to fade to.
    curpal: i32,
    /// `PLZ.C:47`: the timetable index.
    ttptr: usize,
    /// `COPPER.ASM:14`: the drop's step.
    cop_drop: u16,
    /// `COPPER.ASM:19`: the plasma's copper routines are live.
    cop_plz: bool,
    /// `COPPER.ASM:12`: the palette upload pending.
    do_pal: bool,
    /// Whether that upload reads `fadepal` or the cube's `fpal`.
    pal_source: PalSource,
    /// `COPPER.ASM:17`: the CRTC start (`cop_start`).
    cop_start: u32,
    /// `COPPER.ASM:18`: the attribute pan register (`cop_scrl`).
    cop_scrl: u8,
    /// `COPPER.ASM:122`: the split alternation's counter. **1, not 0**: the
    /// engine composes a tick before slot 2 runs, so the source's BSS 0 needs
    /// one retrace of phase to match the reference's frames.
    pompi: u8,
    /// `PLZ.C:49-53`: the live and initial plasma parameters.
    l: [i32; 4],
    k: [i32; 4],
    il: [i32; 4],
    ik: [i32; 4],
    /// `COPPER.ASM:13`: the frame counter the copper increments.
    frame_count: u32,
    /// The CRTC 0x18 line-compare register (8 bits plus the 8th bit).
    line_compare: u16,
    /// Whether `tw_opengraph2` has put the plasma's mode on screen; the copper
    /// runs before that (during the wait) without touching the view.
    plasma_mode: bool,
    /// `VECT.C:36`: the cube's shaded palette, 768 bytes.
    fpal: [u8; 768],
    /// The CRTC state the beam last saw (copper1's write, for the cube's
    /// part-composed window).
    display_start: u32,
    display_pan: u8,
}

impl CopperState {
    fn new() -> CopperState {
        CopperState {
            pals: [[0; 768]; 6],
            dtau: build_dtau(),
            fadepal: [0; 768],
            fade_index: 0,
            curpal: 0,
            ttptr: 0,
            cop_drop: 0,
            cop_plz: true,
            do_pal: false,
            pal_source: PalSource::Fade,
            cop_start: PLASMA_COP_START,
            cop_scrl: 0,
            pompi: 1,
            l: [1000, 2000, 3000, 4000],
            k: [3500, 2300, 3900, 3670],
            il: [1000, 2000, 3000, 4000],
            ik: [3500, 2300, 3900, 3670],
            frame_count: 0,
            line_compare: 60,
            plasma_mode: false,
            fpal: [0; 768],
            display_start: PLASMA_COP_START,
            display_pan: 0,
        }
    }

    /// `COPPER.ASM:93-103`: upload 768 bytes from `cop_pal`.
    fn upload_palette(&self) -> Palette {
        let mut palette = Palette::new();
        let mut bytes = [0u8; 768];
        match self.pal_source {
            PalSource::Fade => {
                for (out, acc) in bytes.iter_mut().zip(self.fadepal.iter()) {
                    // `rep outsb` sends the colour half: the accumulator's
                    // high byte, of which the DAC keeps six bits.
                    *out = dac6((*acc >> 8) as u8);
                }
            }
            PalSource::Cube => {
                for (out, value) in bytes.iter_mut().zip(self.fpal.iter()) {
                    *out = dac6(*value);
                }
            }
        }
        for entry in 0..256usize {
            palette.set_entry(
                entry as u8,
                [bytes[entry * 3], bytes[entry * 3 + 1], bytes[entry * 3 + 2]],
            );
        }
        palette
    }

    /// `COPPER.ASM:126-143`: the split/pan alternation.
    fn pompota(&mut self) {
        self.cop_scrl = 4;
        self.line_compare = 60;
        self.pompi = self.pompi.wrapping_add(1);
        if self.pompi & 1 != 0 {
            self.cop_scrl = 0;
            self.line_compare = 61;
        }
    }

    /// `COPPER.ASM:145-173`: the parameter walk.
    fn moveplz(&mut self) {
        for (value, step) in self.k.iter_mut().zip([-3, -2, 1, 2]) {
            *value = value.wrapping_add(step) & 4095;
        }
        for (value, step) in self.l.iter_mut().zip([-1, -2, 2, 3]) {
            *value = value.wrapping_add(step) & 4095;
        }
    }

    /// `COPPER.ASM:270-297` `initpparas`.
    fn initpparas(&mut self) {
        self.l = self.il;
        self.k = self.ik;
    }

    /// `COPPER.ASM:189-268` `do_drop`.
    fn do_drop(&mut self) {
        self.cop_drop = self.cop_drop.wrapping_add(1);
        if self.cop_drop <= 64 {
            self.line_compare = self.dtau[self.cop_drop as usize];
            return;
        }
        if self.cop_drop >= 256 {
            self.cop_drop = 0;
            return;
        }
        if self.cop_drop > 96 && self.cop_drop < 128 {
            self.cop_drop = 0;
            return;
        }
        // 65..=96 and 128..=255: the fade.
        self.pal_source = PalSource::Fade;
        self.do_pal = true;
        if self.cop_drop == 65 {
            self.line_compare = 400;
            self.initpparas();
            return;
        }
        self.line_compare = 60;
        let source = self.pals[self.fade_index];
        for (acc, step) in self.fadepal.iter_mut().zip(source.iter()) {
            *acc = acc.wrapping_add(*step as u16);
        }
    }
}

/// Four pixels a plane byte: the plasma's `View` stride and its lower view.
fn plasma_lower_view() -> View {
    let mut view = View::new(PLASMA_WIDTH, PLASMA_HEIGHT);
    view.y_line_offset = Some(PLASMA_STRIDE);
    view.border = Some(0);
    view
}

/// Write the CRTC state into the view, when the plasma is the live screen.
fn apply_plasma_view(state: &CopperState, ctx: &mut PartCtx) {
    if !state.cop_plz || !state.plasma_mode {
        return;
    }
    ctx.view.width = PLASMA_WIDTH;
    ctx.view.height = PLASMA_HEIGHT;
    ctx.view.double_scan = false;
    ctx.view.set_origin(state.cop_start * 4);
    ctx.view.y_line_offset = Some(PLASMA_STRIDE);
    // AC 0x13 in a 256-colour mode shifts `(value & 7) / 2` pixels.
    ctx.view.pan = state.cop_scrl / 2;
    ctx.view.border = Some(0);
    ctx.view.split = Some(Box::new(Split {
        scanline: state.line_compare + 1,
        view: plasma_lower_view(),
    }));
}

/// The periodic mask tables (`PLZA.ASM:13-20`), flattened so that
/// `end_mask[x-1]` at `x = 0` reads the byte before `end_mask` — the same
/// `0x0F` the periodic table would give.
fn build_masks() -> Vec<u8> {
    let mut masks = vec![0u8; MASK_TABLE_LEN];
    let start = [0b1111u8, 0b1110, 0b1100, 0b1000];
    let end = [0b0001u8, 0b0011, 0b0111, 0b1111];
    for i in 0..200 {
        masks[START_MASK_AT + i * 4..START_MASK_AT + i * 4 + 4].copy_from_slice(&start);
        masks[END_MASK_AT + i * 4..END_MASK_AT + i * 4 + 4].copy_from_slice(&end);
    }
    masks
}

/// `PLZA.ASM:262-287`: `(source * shade) >> 6`, 192 bytes from `offset`.
fn shadepal(fpal: &mut [u8], pal: &[u8], offset: usize, shade: i32) {
    let shade = shade as u32 as u8;
    let out = &mut fpal[offset..offset + 192];
    let source = &pal[offset..offset + 192];
    for (out, source) in out.iter_mut().zip(source) {
        *out = (((*source as u32) * (shade as u32)) >> 6) as u8;
    }
}

// ------------------------------------------------------------------------
// The part.
// ------------------------------------------------------------------------

impl Part for Plzpart {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // The part gates on the music (`PLZ.C:68`, `VECT.C:116,119`); a run
        // with no player could never leave the gate.
        if ctx.music().is_none() {
            return Err(PartError {
                part: NAME,
                message: "this part waits on the music player's position \
                          (PLZPART/PLZ.C:68, VECT.C:116) and the run has no music to wait on"
                    .into(),
            });
        }
        let tables = Tables::load_at(Path::new(self.table_directory))?;
        let pals = build_pals(&tables);
        {
            let mut state = self.copper.borrow_mut();
            state.pals = pals;
        }
        self.sinit = tables.sinit.clone();
        self.tables = Some(tables);
        self.initvect();

        // The plasma's surface is the four-plane image at four pixels a byte;
        // the window reaches 4*(27,072 + 61*96) + 320.
        let needed =
            4 * (PLASMA_COP_START as usize + 61 * PLASMA_ROW_BYTES) + PLASMA_WIDTH as usize;
        if ctx.surface.pixels().len() < needed {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "the plasma's window reaches surface pixel {needed} and the surface holds {}",
                    ctx.surface.pixels().len()
                ),
            });
        }

        // The entry run's head start. The shell seeks MUSIC1 to order 62
        // (`music_for_part`); the player's published words are eleven polls
        // behind the reference's, and nine pre-polls put the cube's
        // `musplus() >= 13` gate on boot 8364 (part tick 2641, the trace's
        // first order-68 row 13). A mid-chain start has already run through
        // those polls, so it takes none.
        self.fresh = ctx.music_fresh();
        if self.fresh {
            if let Some(music) = ctx.music() {
                for _ in 0..self.music_pre_roll {
                    music.poll();
                }
            }
        }
        // `MAIN.C:8-9`: `init_copper()` runs before `plz()`, so the copper is
        // live through the music wait. Register it here; the fresh run's wait
        // is the measured parameter pre-walk below, a mid-chain run's is the
        // source's `musplus` gate (whose ticks then walk the parameters).
        self.install_copper(ctx);
        if self.fresh {
            let mut state = self.copper.borrow_mut();
            for _ in 0..PLASMA_WAIT_WALKS {
                state.moveplz();
            }
        }
        self.phase = Phase::PlasmaGate;
        self.tick = 0;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        loop {
            match self.phase {
                Phase::PlasmaGate => {
                    // `PLZ.C:68`: `while(dis_musplus()<0 ...)`. The seek has
                    // already put an entry run at the position the reference's
                    // gate was waiting for; the `254` at MUSIC1 order 61 that
                    // gave the reference `zplus == 2` is behind the seek, so a
                    // fresh run's gate is the measured tick 0.
                    let pass = self.fresh || ctx.music().is_none_or(|music| music.musplus() >= 0);
                    if !pass {
                        return PartStatus::Continuing;
                    }
                    if let Some(music) = ctx.music() {
                        music.setmframe(0);
                    }
                    self.init_plz(ctx);
                    self.phase = Phase::Plasma;
                }
                Phase::Plasma => {
                    if self.plasma_tick(ctx) {
                        self.phase = Phase::PlasmaEndWait;
                    } else {
                        return PartStatus::Continuing;
                    }
                }
                Phase::PlasmaEndWait => {
                    // `PLZ.C:119`: `while(frame_count==0);` — the copper's
                    // increment at the end of this tick ends it.
                    if self.copper.borrow().frame_count == 0 {
                        return PartStatus::Continuing;
                    }
                    {
                        let mut state = self.copper.borrow_mut();
                        state.line_compare = 500; // `set_plzstart(500)`
                        state.cop_plz = false;
                        state.frame_count = 0;
                    }
                    self.set_cube_mode(ctx);
                    self.phase = Phase::CubeGate;
                }
                Phase::CubeGate => {
                    // `VECT.C:116`: `while(dis_musplus()<13 ...)`.
                    let reading = ctx.music().map_or(-32, |music| music.musplus());
                    if reading < CUBE_GATE {
                        return PartStatus::Continuing;
                    }
                    self.copper.borrow_mut().frame_count = 0;
                    // `VECT.C:116-117`: `while(musplus<13...); frame_count=0;`
                    // and the loop's first `swappage()` are the same frame
                    // period — the page being drawn next tick must already be
                    // the one the copper puts on screen.
                    self.swap_page();
                    self.phase = Phase::Cube { swapped: true };
                }
                Phase::Cube { swapped } => {
                    return self.cube_tick(ctx, swapped);
                }
                Phase::Done => return PartStatus::Finished,
            }
        }
    }
}

impl Plzpart {
    /// `PLZ.C:124-218` `init_plz`, after the gate: the mode set, the display
    /// state, the three copper routines and the five palette tables.
    fn init_plz(&mut self, ctx: &mut PartCtx) {
        // `TWEAK.ASM:44-67` `tw_opengraph2`: chain-4 off and the 64 KB clear
        // (all four planes). The surface is the four-plane image.
        ctx.surface.pixels_mut().fill(0);
        // `PLZ.C:179`: `for(a=0;a<256;a++) tw_setrgbpalette(a,63,63,63)`.
        let mut white = Palette::new();
        for entry in 0..=255u8 {
            white.set_entry(entry, [63, 63, 63]);
        }
        ctx.set_palette(white);
        {
            let mut state = self.copper.borrow_mut();
            state.cop_start = PLASMA_COP_START;
            state.cop_scrl = 0;
            state.line_compare = 60; // `set_plzstart(60)`
            state.plasma_mode = true;
            state.cop_drop = 128;
            state.cop_plz = true;
            state.do_pal = false;
            state.pal_source = PalSource::Fade;
            state.fadepal = [0; 768];
            // `cop_fadepal = pals[curpal++]`.
            state.fade_index = 0;
            state.curpal = 1;
            state.ttptr = 0;
            state.pompi = 1;
            state.frame_count = 0;
            // `l`/`k` are *not* reset: `init_plz` leaves the globals the wait
            // has walked, and `il`/`ik` keep the initial values `initpparas`
            // restores.
            state.display_start = PLASMA_COP_START;
            state.display_pan = 0;
        }
        apply_plasma_view(&self.copper.borrow(), ctx);
    }

    /// `COPPER.ASM:44-65` `init_copper`: the three registrations.
    fn install_copper(&mut self, ctx: &mut PartCtx) {
        // Slot 0: `copper3`, empty (`COPPER.ASM:118-120`).
        ctx.set_copper(0, Box::new(|_ctx| {}));
        // Slot 1: `copper1`, the CRTC start/pan writer (`:67-84`).
        let shared = Rc::clone(&self.copper);
        ctx.set_copper(
            1,
            Box::new(move |ctx| {
                let mut state = shared.borrow_mut();
                // The `View` is the beam; the write takes effect on the next
                // compose, which is the retrace's own boundary.
                state.display_start = state.cop_start;
                state.display_pan = state.cop_scrl / 2;
                apply_plasma_view(&state, ctx);
            }),
        );
        // Slot 2: `copper2`, the frame counter, the upload, pompota/moveplz
        // and do_drop (`:86-116`).
        let shared = Rc::clone(&self.copper);
        ctx.set_copper(
            2,
            Box::new(move |ctx| {
                let mut state = shared.borrow_mut();
                state.frame_count = state.frame_count.wrapping_add(1);
                if state.do_pal {
                    let palette = state.upload_palette();
                    ctx.set_palette(palette);
                    state.do_pal = false;
                }
                if state.cop_plz {
                    state.pompota();
                    state.moveplz();
                }
                if state.cop_drop != 0 {
                    state.do_drop();
                }
                apply_plasma_view(&state, ctx);
            }),
        );
    }

    /// `PLZ.C:76-118`: one pass of the plasma's loop. `true` when it breaks.
    fn plasma_tick(&mut self, ctx: &mut PartCtx) -> bool {
        let published = ctx.music().map_or(0, |music| music.getmframe());
        let mut state = self.copper.borrow_mut();
        // `tim+=frame_count; frame_count=0; count++;`
        state.frame_count = 0;
        if state.ttptr < TIMETABLE.len() && published > TIMETABLE[state.ttptr] {
            // `memset(fadepal,0,768)` zeroes the colour half and leaves the
            // fraction: the palette leak.
            for acc in state.fadepal.iter_mut() {
                *acc &= 0x00FF;
            }
            state.cop_drop = 1;
            state.fade_index = state.curpal as usize;
            state.curpal += 1;
            state.ttptr += 1;
            let row = INITTABLE[state.ttptr.min(INITTABLE.len() - 1)];
            state.il = [row[0], row[1], row[2], row[3]];
            state.ik = [row[4], row[5], row[6], row[7]];
            state.l = state.il;
            state.k = state.ik;
        }
        if state.curpal == EXIT_PALETTE_RELEASE && state.cop_drop > 64 {
            state.cop_drop = 0;
            state.frame_count = 0;
            return true;
        }
        drop(state);
        self.draw_plasma(ctx);
        false
    }

    /// The four plane passes of `PLZ.C:96-117`.
    fn draw_plasma(&self, ctx: &mut PartCtx) {
        let tables = self.tables.as_ref().expect("init loaded the tables");
        let (k, l) = {
            let state = self.copper.borrow();
            (state.k, state.l)
        };
        let surface = ctx.surface.pixels_mut();
        // The reference's filed plasma frames are **tears**: `plz()`'s loop
        // has no frame wait, so the copper's once-per-vblank `moveplz` can
        // latch a second parameter generation between the four passes (the
        // split table is in A24). The port's fixed tick draws the loop's
        // phase-locked steady state: one generation for all four passes.
        for (start, params, planes) in [
            (0usize, k, [1usize, 3]),
            (1, l, [1, 3]),
            (1, k, [0, 2]),
            (0, l, [0, 2]),
        ] {
            for y in (start..PLASMA_ROWS).step_by(2) {
                let line = plasma_line(tables, y, params[0], params[1], params[2], params[3]);
                for (r, value) in line.iter().enumerate() {
                    let at = 4 * (y * PLASMA_ROW_BYTES + r);
                    for &plane in &planes {
                        surface[at + plane] = *value;
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ the cube --

    /// `PLZFILL.C:25-68` `initvect`.
    fn initvect(&mut self) {
        self.video = vec![0; VIDEO_BYTES];
        let mut sini = vec![0i16; 2000];
        for (index, value) in sini.iter_mut().enumerate().take(1524) {
            let angle = index as f64 / 1024.0 * std::f64::consts::PI * 4.0;
            *value = (angle.sin() * 127.0) as i16;
        }
        self.sini = sini;
        let sini = &self.sini;
        // `PLZFILL.C:37-55`: the three colour bands.
        let mut pal = [0u8; 768];
        for a in 1..32usize {
            pal[a * 3] = 0;
            pal[a * 3 + 1] = 0;
            pal[a * 3 + 2] = (a * 2) as u8;
        }
        for a in 0..32usize {
            pal[a * 3 + 32 * 3] = (a * 2) as u8;
            pal[a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[a * 3 + 2 + 32 * 3] = 63;
        }
        for a in 0..32usize {
            pal[192 + a * 3] = (a * 2) as u8;
            pal[192 + a * 3 + 1] = 0;
            pal[192 + a * 3 + 2] = 0;
        }
        for a in 0..32usize {
            pal[192 + a * 3 + 32 * 3] = 63;
            pal[192 + a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[192 + a * 3 + 2 + 32 * 3] = 0;
        }
        for a in 0..32usize {
            pal[384 + a * 3] = a as u8;
            pal[384 + a * 3 + 1] = 0;
            pal[384 + a * 3 + 2] = (a * 2 / 3) as u8;
        }
        for a in 0..32usize {
            pal[384 + a * 3 + 32 * 3] = (31 - a) as u8;
            pal[384 + a * 3 + 1 + 32 * 3] = (a * 2) as u8;
            pal[384 + a * 3 + 2 + 32 * 3] = 21;
        }
        self.pal = pal;
        // `PLZFILL.C:57-62`: the three textures, 64 rows of a 256-byte stride
        // (the linked segment is 16,384 bytes, `INCLUDE.ASM:5-9`).
        let mut kuva1 = vec![0u8; 16_384];
        let mut kuva2 = vec![0u8; 16_384];
        let mut kuva3 = vec![0u8; 16_384];
        for y in 0..64usize {
            for x in 0..256usize {
                let index = ((y as i32 * 4 + sini[x * 2] as i32) & 511) as usize;
                let value = sini[index] / 4 + 32;
                kuva1[y * 256 + x] = value as u8;
                kuva2[y * 256 + x] = (value + 64) as u8;
                kuva3[y * 256 + x] = (value + 128) as u8;
            }
        }
        self.kuva1 = kuva1;
        self.kuva2 = kuva2;
        self.kuva3 = kuva3;
        // `PLZFILL.C:64-65`: `dist1[y][x] = sini[y*8]/3`, a 256-byte row.
        let mut dist1 = vec![0u8; 128 * 256];
        for y in 0..128usize {
            let value = (sini[y * 8] as i32 / 3) as i8;
            for x in 0..256usize {
                dist1[y * 256 + x] = value as u8;
            }
        }
        self.dist1 = dist1;
        // `PLZFILL.C:67`: every edge-list slot starts empty.
        self.clrtau = vec![[640, 0]; CLR_SLOTS * 256];
        self.clrptr = 0;
    }

    /// `VECT.C:114-116` `tw_opengraph`, then the wait's display state: the
    /// part composes the cube's beam itself (module docs).
    fn set_cube_mode(&mut self, ctx: &mut PartCtx) {
        // `TWEAK.ASM:18-37`: chain-4 off and the 64 KB clear, all planes.
        self.video.fill(0);
        ctx.view.width = CUBE_WIDTH;
        ctx.view.height = CUBE_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(640);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
        self.page = 0;
        self.frames = 0;
        self.clrptr = 0;
        self.clrtau.iter_mut().for_each(|entry| *entry = [640, 0]);
    }

    /// `VECT.C:117-128`: one pass of the cube's loop.
    fn cube_tick(&mut self, ctx: &mut PartCtx, swapped: bool) -> PartStatus {
        let mut swapped = swapped;
        if !swapped {
            // `a=dis_musplus(); if(a>=-4 && a<0) break;`
            let reading = ctx.music().map_or(-32, |music| music.musplus());
            if (CUBE_EXIT_LOW..0).contains(&reading) {
                self.phase = Phase::Done;
                return PartStatus::Finished;
            }
            self.swap_page();
            swapped = true;
            self.phase = Phase::Cube { swapped };
        }
        let count = self.copper.borrow().frame_count;
        if count < 1 {
            // `while(frame_count<1);` — one tick in the port.
            self.compose_cube(ctx);
            return PartStatus::Continuing;
        }
        {
            let mut state = self.copper.borrow_mut();
            state.frame_count = 0;
            state.pal_source = PalSource::Cube;
            state.do_pal = true;
        }
        self.frames += count as i32;
        self.calculate();
        self.draw_cube();
        self.clear();
        self.compose_cube(ctx);
        // The loop repeats within the same frame boundary: check the music,
        // then swap for the next iteration and wait for the next vblank.
        let reading = ctx.music().map_or(-32, |music| music.musplus());
        if (CUBE_EXIT_LOW..0).contains(&reading) {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        self.swap_page();
        self.phase = Phase::Cube { swapped: true };
        PartStatus::Continuing
    }

    /// `VECT.C:265-298` `swappage`.
    fn swap_page(&mut self) {
        self.page = (self.page + 1) % 6;
        let mut state = self.copper.borrow_mut();
        state.cop_start = u32::from(PAGE_START[self.page]);
        state.cop_scrl = PAGE_PAN[self.page];
    }

    /// The beam's 320×200 window over the four-plane image.
    fn compose_cube(&self, ctx: &mut PartCtx) {
        let (start, pan) = {
            let state = self.copper.borrow();
            (state.display_start, state.display_pan)
        };
        let base = 4 * start + u32::from(pan);
        let surface = ctx.surface.pixels_mut();
        for y in 0..CUBE_HEIGHT as usize {
            let row = &mut surface[y * 640..y * 640 + CUBE_WIDTH as usize];
            for (x, out) in row.iter_mut().enumerate() {
                let linear = (base + (y as u32) * 640 + x as u32) & VIDEO_MASK;
                *out = self.video[linear as usize];
            }
        }
    }

    /// `VECT.C:132-151` `calculate`.
    fn calculate(&mut self) {
        let tables = self.tables.as_ref().expect("init loaded the tables");
        let position = ((4 * 256 + self.frames * 4) & 0xFFFF) as usize;
        let segment = position >> 8;
        let fraction = position & 255;
        let mut out = [0i32; 8];
        for (word, value) in out.iter_mut().enumerate() {
            let control = |offset: usize| tables.rata[segment * 8 + offset] as i32;
            let coefficient = |offset: usize| tables.spline[fraction + offset] as i32;
            let sum = control(24 + word)
                .wrapping_mul(coefficient(0))
                .wrapping_add(control(16 + word).wrapping_mul(coefficient(256)))
                .wrapping_add(control(8 + word).wrapping_mul(coefficient(512)))
                .wrapping_add(control(word).wrapping_mul(coefficient(768)));
            *value = sum >> 15;
        }
        // `SPLINE.ASM:52-75`: the pops are ls_ky, ls_kx, kz, ky, kx, dis, ty, tx.
        self.tx = out[0];
        self.ty = out[1];
        self.dis = out[2];
        self.kx = out[3] & 1023;
        self.ky = out[4] & 1023;
        self.kz = out[5] & 1023;
        self.ls_kx = out[6] & 1023;
        self.ls_ky = out[7] & 1023;
        // `VECT.C:144-146`.
        // `kosinit = sinit + 512` is 512 **bytes** — 256 words, the cosine:
        // the map puts `_kosinit` at `2CE4:0200` (`PLZPART.MAP:353`).
        let sx = self.sinit[self.ls_kx as usize] as i32 >> 8;
        let sy = self.sinit[self.ls_ky as usize] as i32 >> 8;
        let cy = self.sinit[self.ls_ky as usize + 256] as i32 >> 8;
        self.ls_y = cy;
        self.ls_x = (sx.wrapping_mul(sy)) >> 7;
        self.ls_z = (sx.wrapping_mul(cy)) >> 7;
        self.count_const();
        self.rotate();
        self.sort_faces();
    }

    fn sine(&self, index: i32) -> i32 {
        self.sinit[index as usize] as i32
    }

    /// `VECT.C:166-176` `count_const`, with the source's precedence:
    /// `>>15+7` is `>>22`, and the sums are inside the final shift where the
    /// source puts them (`VECT.OBJ` disassembles to `sar $0x16`).
    fn count_const(&mut self) {
        // `kosinit[k]` is the word 256 entries past `sinit[k]`: `kosinit =
        // sinit + 512` adds 512 bytes to the label, and the array is words
        // (`PLZPART.MAP:353`, `INCLUDE.ASM:36`).
        let sx = self.sine(self.kx);
        let cx = self.sine(self.kx + 256);
        let sy = self.sine(self.ky);
        let cy = self.sine(self.ky + 256);
        let sz = self.sine(self.kz);
        let cz = self.sine(self.kz + 256);
        self.cxx = cy.wrapping_mul(cz) >> 22;
        self.cxy = cy.wrapping_mul(sz) >> 22;
        self.cxz = (-sy) >> 7;
        self.cyx = ((sx.wrapping_mul(cz).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(sy)
            .wrapping_sub(cx.wrapping_mul(sz))
            >> 22;
        self.cyy = ((sx.wrapping_mul(sy).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(sz)
            .wrapping_add(cx.wrapping_mul(cz))
            >> 22;
        self.cyz = cy.wrapping_mul(sx) >> 22;
        self.czx = ((cx.wrapping_mul(cz).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(sy)
            .wrapping_add(sx.wrapping_mul(sz))
            >> 22;
        self.czy = ((cx.wrapping_mul(sy).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(sz)
            .wrapping_sub(sx.wrapping_mul(cz))
            >> 22;
        self.czz = cy.wrapping_mul(cx) >> 22;
    }

    /// `VECT.C:180-201` `rotate`, in the assembly's 16-bit arithmetic: every
    /// product is an `imulw`, its low word shifted, the sums 16-bit, and the
    /// projection `xxx = (xx*256)/zz + 160 + 160`.
    fn rotate(&mut self) {
        let add16 = |a: i32, b: i32| -> i32 { (a.wrapping_add(b) as i16) as i32 };
        let term = |value: i32, coefficient: i32| -> i32 {
            ((value.wrapping_mul(coefficient) as i16) as i32) >> 1
        };
        for index in 0..self.points.len() {
            let [x, y, z] = self.points[index];
            let xx = add16(
                add16(term(x, self.cxx), term(y, self.cxy)),
                term(z, self.cxz),
            ) >> 7;
            let xx = (xx.wrapping_add(self.tx) as i16) as i32;
            let yy = add16(
                add16(term(x, self.cyx), term(y, self.cyy)),
                term(z, self.cyz),
            ) >> 7;
            let yy = (yy.wrapping_add(self.ty) as i16) as i32;
            let zz = add16(
                add16(term(x, self.czx), term(y, self.czy)),
                term(z, self.czz),
            ) >> 7;
            let zz = (zz.wrapping_add(self.dis) as i16) as i32;
            self.xx[index] = [xx, yy, zz];
            let xxx = xx.wrapping_mul(256) / zz + 160 + 160;
            // The release's vertical projection origin and scale, measured
            // against fourteen captured frames (D24, APPROXIMATIONS A23): the
            // tree's development build compiles 142/66 (`VECT.OBJ`:
            // `imul eax,eax,0x8e; add ax,0x42`), but the release's cube sits
            // 1.5x taller and is centred on the screen. With 213/100 the six
            // consecutive captures 8400-8800 align within 2 px of (0,0) at
            // 0.44-0.79 % (8900/9000 at 0.73/0.07 %); with 142/66 the same
            // frames differ by 3.0-10.6 % after a shift.
            let yyy = yy.wrapping_mul(213) / zz + 100;
            self.xxx[index] = [xxx, yyy];
        }
    }

    /// `VECT.C:203-246` `sort_faces`.
    fn sort_faces(&mut self) {
        let mut a = 0usize;
        let mut p = 0usize;
        while a < self.pg.len() {
            let pg = self.pg[a];
            let p1 = &self.xx[pg[0] as usize];
            let (x, y, z) = (p1[0], p1[1], p1[2]);
            let p2 = &self.xx[pg[1] as usize];
            let ax = p2[0].wrapping_sub(x);
            let ay = p2[1].wrapping_sub(y);
            let az = p2[2].wrapping_sub(z);
            let p3 = &self.xx[pg[2] as usize];
            let bx = p3[0].wrapping_sub(x);
            let by = p3[1].wrapping_sub(y);
            let bz = p3[2].wrapping_sub(z);
            let nx = ay.wrapping_mul(bz).wrapping_sub(az.wrapping_mul(by));
            let ny = az.wrapping_mul(bx).wrapping_sub(ax.wrapping_mul(bz));
            let nz = ax.wrapping_mul(by).wrapping_sub(ay.wrapping_mul(bx));
            let kx = -x;
            let ky = -y;
            let kz = -z;
            let s = kx
                .wrapping_mul(nx)
                .wrapping_add(ky.wrapping_mul(ny))
                .wrapping_add(kz.wrapping_mul(nz));
            if s > 0 {
                a += 1;
                continue;
            }
            let shade = (self
                .ls_x
                .wrapping_mul(nx)
                .wrapping_add(self.ls_y.wrapping_mul(ny))
                .wrapping_add(self.ls_z.wrapping_mul(nz)))
                / 250_000
                + 32;
            self.light_src[p] = shade;
            let color = pg[7] as usize;
            if self.lls[p] != shade {
                let mut fpal = self.copper.borrow().fpal;
                shadepal(&mut fpal, &self.pal, color * 64 * 3, shade);
                self.copper.borrow_mut().fpal = fpal;
                self.lls[p] = shade;
            }
            self.ptodraw[p] = a as i32;
            p += 1;
            a += 1;
        }
        self.polys = p as i32;
    }

    /// `VECT.C:249-263` `draw`.
    fn draw_cube(&mut self) {
        let shift = (self.page & 1) as i32 * 2;
        for i in 0..self.polys as usize {
            let face = self.ptodraw[i] as usize;
            let pg = self.pg[face];
            let point = |index: i32| -> (i32, i32) {
                let at = index as usize;
                (self.xxx[at][0] + shift, self.xxx[at][1])
            };
            let coords = [point(pg[0]), point(pg[1]), point(pg[2]), point(pg[3])];
            self.do_poly(coords, pg[7] as usize, self.frames & 63);
        }
    }

    /// `PLZFILL.C:82-176` `do_poly`: the span setup.
    fn do_poly(&mut self, pnts: [(i32, i32); 4], color: usize, dd: i32) {
        let dd = (dd + 1) & 63;
        let txt = TEXTURE_POINTS;
        let mut n = 0usize;
        for a in 1..4 {
            if pnts[a].1 < pnts[n].1 {
                n = a;
            }
        }
        let mut s1 = n;
        let mut s2 = n;
        let mut d1 = (s1 + 1) & 3;
        let mut d2 = (s2 + 3) & 3;
        self.setup_edge(&pnts, &txt, s1, d1, 1);
        self.setup_edge(&pnts, &txt, s2, d2, 2);
        self.ryy = pnts[s1].1;
        self.from_texture = color;
        self.dseg_row = dd as usize;
        self.to_row = i32::from(PAGE_VMEM[self.page]) + self.ryy * CUBE_ROW_BYTES as i32;
        self.ctau_row = self.ryy;
        let mut count = 0;
        while count < 4 {
            let m = pnts[d1].1.min(pnts[d2].1);
            self.do_block(m - self.ryy);
            self.ryy = m;
            if pnts[d1].1 == pnts[d2].1 {
                s1 = d1;
                d1 = (s1 + 1) & 3;
                s2 = d2;
                d2 = (s2 + 3) & 3;
                count += 2;
                self.setup_edge(&pnts, &txt, s1, d1, 1);
                self.setup_edge(&pnts, &txt, s2, d2, 2);
            } else if pnts[d1].1 < pnts[d2].1 {
                s1 = d1;
                d1 = (s1 + 1) & 3;
                count += 1;
                self.setup_edge(&pnts, &txt, s1, d1, 1);
            } else {
                s2 = d2;
                d2 = (s2 + 3) & 3;
                count += 1;
                self.setup_edge(&pnts, &txt, s2, d2, 2);
            }
        }
    }

    fn setup_edge(
        &mut self,
        pnts: &[(i32, i32); 4],
        txt: &[[i32; 2]; 4],
        s: usize,
        d: usize,
        which: usize,
    ) {
        let dx = pnts[d].0.wrapping_sub(pnts[s].0);
        let mut dy = pnts[d].1.wrapping_sub(pnts[s].1);
        if dy == 0 {
            dy = 1;
        }
        let slope = 65_536i32.wrapping_mul(dx) / dy;
        let xx = pnts[s].0.wrapping_shl(16).wrapping_add(0x8000);
        let txx = txt[s][0].wrapping_shl(16).wrapping_add(0x8000);
        let txy = txt[s][1].wrapping_shl(16).wrapping_add(0x8000);
        let tax = 65_536i32.wrapping_mul(txt[d][0].wrapping_sub(txt[s][0])) / dy;
        let tay = 65_536i32.wrapping_mul(txt[d][1].wrapping_sub(txt[s][1])) / dy;
        if which == 1 {
            self.ax1 = slope;
            self.xx1 = xx;
            self.txx1 = txx;
            self.txy1 = txy;
            self.tax1 = tax;
            self.tay1 = tay;
        } else {
            self.ax2 = slope;
            self.xx2 = xx;
            self.txx2 = txx;
            self.txy2 = txy;
            self.tax2 = tax;
            self.tay2 = tay;
        }
    }

    /// One byte to all selected planes at a 16-bit plane-byte address.
    fn write_plane_byte(&mut self, address: u16, mask: u8, value: u8) {
        for plane in 0..4usize {
            if mask & (1 << plane) != 0 {
                self.video[4 * address as usize + plane] = value;
            }
        }
    }

    /// The clock at a texture address: `dist1[dd*16 + bx]` (unsigned) and then
    /// the texture byte at `bx + dist` (`PLZA.ASM:130-166`).
    fn texture_sample(&self, bx: u16) -> u8 {
        // `dseg = disttaus[color] + dd*16` adds **16 paragraphs** (256 bytes)
        // to the segment, so the warp shifts `dist1` by `dd` 256-byte rows.
        let dist = self.dist1[self.dseg_row * 256 + bx as usize];
        let texture = match self.from_texture {
            0 => &self.kuva1,
            1 => &self.kuva2,
            _ => &self.kuva3,
        };
        let address = bx.wrapping_add(u16::from(dist)) as usize;
        if address >= texture.len() {
            // The texture is 128×256 and the dist warp stays in its half;
            // a read past it is the original's segment wrap, not a sample.
            return 0;
        }
        texture[address]
    }

    /// `PLZA.ASM:59-259` `do_block`.
    fn do_block(&mut self, ycount: i32) {
        if ycount == 0 {
            return;
        }
        let mut lines = ycount;
        let mut to = self.to_row;
        let mut ctau = self.ctau_row;
        loop {
            if self.ryy >= 0 && self.ryy < CUBE_DRAWN_ROWS {
                self.block_line(to as u16, ctau);
            } else if self.ryy >= CUBE_DRAWN_ROWS {
                break;
            }
            to = to.wrapping_add(CUBE_ROW_BYTES as i32);
            self.ryy += 1;
            ctau += 1;
            self.xx1 = self.xx1.wrapping_add(self.ax1);
            self.xx2 = self.xx2.wrapping_add(self.ax2);
            self.txy1 = self.txy1.wrapping_add(self.tay1);
            self.txx1 = self.txx1.wrapping_add(self.tax1);
            self.txy2 = self.txy2.wrapping_add(self.tay2);
            self.txx2 = self.txx2.wrapping_add(self.tax2);
            lines -= 1;
            if lines == 0 {
                break;
            }
        }
        self.to_row = to;
        self.ctau_row = ctau;
    }

    /// One scanline's span (`PLZA.ASM:73-192`).
    fn block_line(&mut self, to: u16, _ctau: i32) {
        let row = self.ryy as usize;
        let right = (self.xx2 as u32 >> 16) as u16;
        let left = (self.xx1 as u32 >> 16) as u16;
        // `PLZA.ASM:73-90`: `ctau[0]` is the *smallest* span start (`cmp ax,
        // [gs:bp]` / `ja` keeps the smaller) and `ctau[1]` the *largest* span
        // end — the row's union hull, which is what `do_clear` trims against.
        let entry = self.clrptr * 256 + row;
        if right <= self.clrtau[entry][0] as u16 {
            self.clrtau[entry][0] = i32::from(right);
        }
        if left >= self.clrtau[entry][1] as u16 {
            self.clrtau[entry][1] = i32::from(left);
        }
        let start_col = right >> 2;
        let end_col = left >> 2;
        if end_col < start_col {
            return;
        }
        let bytes = end_col - start_col;
        let di = to.wrapping_add(start_col);
        if bytes == 0 {
            // `@@singlebyte`: the start and end masks combined.
            let sample = self.texture_sample(texture_address(self.txy2, self.txx2));
            let mask = self.masks[START_MASK_AT + usize::from(right & 3)]
                & self.masks[END_MASK_AT + usize::from(left & 3)];
            self.write_plane_byte(di, mask, sample);
            return;
        }
        // `@@twobyte`: the two end bytes first.
        let cl = self.texture_sample(texture_address(self.txy2, self.txx2));
        let ch = self.texture_sample(texture_address(self.txy1, self.txx1));
        self.write_plane_byte(di, self.masks[START_MASK_AT + usize::from(right & 3)], cl);
        self.write_plane_byte(
            di.wrapping_add(bytes),
            self.masks[END_MASK_AT + usize::from(left & 3)],
            ch,
        );
        if bytes >= 2 {
            self.fill_span(di, bytes);
        }
    }

    /// The texture address the assembly forms from a 16.16 pair:
    /// `bh = ty`, `bl = tx` (`PLZA.ASM:125-126`).
    fn fill_span(&mut self, di: u16, bytes: u16) {
        // `mov bp,si; dec si; ... movsx esi,si`: the divisor is **bytes-1**,
        // the count of middle bytes, not the span length.
        let divisor = bytes as i32 - 1;
        let addy = self.txy2.wrapping_sub(self.txy1) / divisor;
        let addx = self.txx2.wrapping_sub(self.txx1) / divisor;
        // `mov ebx,[txy1-2]`: the y fraction in the high word, tx in the low.
        let mut ebx = (self.txy1 as u32 & 0xFFFF) << 16 | ((self.txx1 as u32 >> 16) & 0xFFFF);
        let mut ecx = addy as u32;
        let eax = (addx as u32).rotate_left(16);
        let rotated = (addy as u32).rotate_left(16);
        // `mov dh,dl`: dh becomes the rotated low byte.
        let mut dh = (rotated & 0xFF) as u8;
        // `mov dl,al`: dl becomes the rotated addx's low byte.
        let dl = (eax & 0xFF) as u8;
        let mut edx = (rotated & 0xFFFF_0000) | ((dh as u32) << 8) | (dl as u32);
        if dl & 0x80 != 0 {
            dh = dh.wrapping_sub(1);
            // Clear both bytes dh occupies: the mask must keep only the top
            // 16 bits (the y fraction); `0xFFFF_FF00` would leave the old
            // dh behind and OR the corrected one into it.
            edx = (edx & 0xFFFF_00FF) | ((dh as u32) << 8) | (dl as u32);
        }
        // The address bytes come from `txy1+2` / `txx1+2`.
        ebx = (ebx & 0xFFFF_0000)
            | (((self.txy1 as u32 >> 16) & 0xFF) << 8)
            | ((self.txx1 as u32 >> 16) & 0xFF);
        // The middle bytes, right to left (`PLZA.ASM:148-166`).
        let mut position = di.wrapping_add(bytes).wrapping_sub(1);
        for _step in 0..bytes.saturating_sub(1) {
            let bx = ebx as u16;
            let sample = self.texture_sample(bx);
            self.write_plane_byte(position, 0x0F, sample);
            position = position.wrapping_sub(1);
            // `add ecx,eax` sets the carry the following `adc ebx,edx` uses.
            let (sum, carry1) = ecx.overflowing_add(eax);
            ecx = sum;
            let (sum, carry2a) = ebx.overflowing_add(edx);
            let (sum, carry2b) = sum.overflowing_add(u32::from(carry1));
            ebx = sum;
            let carry = carry2a || carry2b;
            // `adc bh,0`: the carry out of the 32-bit sum lands in ty.
            let bh = ((ebx >> 8) & 0xFF) as u8;
            ebx = (ebx & 0xFFFF_00FF) | (u32::from(bh.wrapping_add(u8::from(carry))) << 8);
        }
    }

    /// `PLZFILL.C:178-185` `clear`.
    fn clear(&mut self) {
        let old = (self.clrptr + CLR_SLOTS - 3) % CLR_SLOTS;
        let new = self.clrptr;
        self.clrptr = (self.clrptr + 1) % CLR_SLOTS;
        self.do_clear(old, new);
    }

    /// `PLZA.ASM:291-372` `do_clear`.
    fn do_clear(&mut self, old_slot: usize, new_slot: usize) {
        // `les dx,[vmem]` puts the drawn page's offset in dx, then
        // `mov dl,[es:0]` overwrites its low byte with the byte at the start
        // of plane memory. The original's bug; kept. The high byte still
        // carries the page (0x5500/0xAA00), which is what aims the fill at
        // the buffer the draw used.
        let mut dx = (PAGE_VMEM[self.page] & 0xFF00) | u16::from(self.video[0]);
        for line in 0..CLEAR_LINES as usize {
            let old = self.clrtau[old_slot * 256 + line];
            let new = self.clrtau[new_slot * 256 + line];
            if old[0] != 640 {
                let from = (old[0] as u16) >> 2;
                let to = (new[0] as u16) >> 2;
                if to >= from {
                    let mut at = from.wrapping_add(dx);
                    for _ in 0..(to - from) {
                        self.write_plane_byte(at, 0x0F, 0);
                        at = at.wrapping_add(1);
                    }
                }
                let from = (new[1] as u16) >> 2;
                let to = (old[1] as u16) >> 2;
                if to >= from {
                    let mut at = from.wrapping_add(dx).wrapping_add(1);
                    for _ in 0..(to - from) {
                        self.write_plane_byte(at, 0x0F, 0);
                        at = at.wrapping_add(1);
                    }
                }
            }
            self.clrtau[old_slot * 256 + line] = [640, 0];
            dx = dx.wrapping_add(CUBE_ROW_BYTES as u16);
        }
        // `PLZA.ASM:345-368`: the two edge bytes of each span. `les di,[vmem]`
        // starts the pointer at the drawn page, then `add di,160` a line.
        let mut offset = PAGE_VMEM[self.page];
        for line in 0..(CLEAR_LINES - 1) as usize {
            let new = self.clrtau[new_slot * 256 + line];
            // `end_mask[x-1]` — at x = 0 the 16-bit displacement wraps to the
            // byte before end_mask, which is start_mask's last 0x0F.
            let x = (new[0] as u16).wrapping_sub(1);
            let mask = self.masks[(END_MASK_AT as u16).wrapping_add(x) as usize];
            self.write_plane_byte(offset.wrapping_add(x >> 2), mask, 0);
            let x = (new[1] as u16).wrapping_add(1);
            let mask = self.masks[(START_MASK_AT as u16).wrapping_add(x) as usize];
            self.write_plane_byte(offset.wrapping_add(x >> 2), mask, 0);
            offset = offset.wrapping_add(CUBE_ROW_BYTES as u16);
        }
    }
}

/// The texture address `bh<<8 | bl` from a 16.16 pair (`PLZA.ASM:125-126`).
fn texture_address(txy: i32, txx: i32) -> u16 {
    (((txy as u32 >> 16) & 0xFF) << 8 | ((txx as u32 >> 16) & 0xFF)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn tables() -> Tables {
        Tables::load_at(&root().join("assets/PLZPART")).expect("the seven fragments load")
    }

    #[test]
    fn the_inc_fragments_parse_to_their_original_sizes_and_edges() {
        let t = tables();
        assert_eq!(t.psini.len(), PSINI_LEN);
        assert_eq!((t.psini[0], t.psini[1], t.psini[2]), (64, 64, 64));
        assert_eq!(&t.psini[PSINI_LEN - 4..], &[62, 63, 63, 63]);
        assert_eq!(t.lsini4.len(), LSINI_WORDS);
        assert_eq!((t.lsini4[0], t.lsini4[1]), (512, 513));
        assert_eq!(t.lsini4[LSINI_WORDS - 1], 510);
        assert_eq!(t.lsini16.len(), LSINI_WORDS);
        assert_eq!((t.lsini16[0], t.lsini16[1]), (1024, 1027));
        assert_eq!(t.lsini16[LSINI_WORDS - 1], 1020);
        // SINIT.INC is 2,575 db bytes: the 1,024-word table and the stray
        // repeat that `INCLUDE.ASM` links.
        assert_eq!(t.sinit.len(), SINIT_WORDS);
        assert_eq!((t.sinit[0], t.sinit[1], t.sinit[256]), (0, 201, 32766));
        assert_eq!(t.ptau.len(), PTAU_LEN);
        assert_eq!((t.ptau[0], t.ptau[1], t.ptau[128]), (0, 1, 1));
        assert_eq!(t.spline.len(), SPLINE_WORDS);
        assert_eq!(&t.spline[..4], &[0, 0, 0, 0]);
        assert_eq!(t.rata.len(), RATA_WORDS);
        assert_eq!(&t.rata[..8], &[0, 2000, 500, 0, 400, 600, 0, 0]);
    }

    /// The hard-coded displacements in `PLZA.ASM` are placeholders inside
    /// `plzline`, and the map puts the tables elsewhere. Read the map itself, so
    /// the claim is reproducible from the file the note cites.
    #[test]
    fn the_table_bases_are_the_maps_not_the_assembly_placeholders() {
        let map_bytes = std::fs::read(root().join("upstream/SecondReality/PLZPART/PLZPART.MAP"))
            .expect("the map is in the vendored tree");
        let map = String::from_utf8_lossy(&map_bytes);
        let find = |symbol: &str| -> u16 {
            for line in map.lines() {
                let tokens: Vec<&str> = line.split_whitespace().collect();
                if tokens.last() != Some(&symbol) {
                    continue;
                }
                if let Some(address) = tokens.first() {
                    if let Some((_segment, offset)) = address.split_once(':') {
                        if let Ok(offset) = u16::from_str_radix(offset, 16) {
                            return offset;
                        }
                    }
                }
            }
            panic!("{symbol} is in the map");
        };
        assert_eq!(find("_psini"), MAP_PSINI);
        assert_eq!(find("_lsini4"), MAP_LSINI4);
        assert_eq!(find("_lsini16"), MAP_LSINI16);
        assert_eq!(find("_plzline"), MAP_PLZLINE);
        // And the placeholders sit in `plzline`'s code, not at those bases.
        for placeholder in PLACEHOLDERS {
            assert!(
                placeholder > MAP_PLZLINE,
                "0x{placeholder:04X} is inside plzline"
            );
        }
        let asm_bytes =
            std::fs::read(root().join("upstream/SecondReality/PLZPART/ASMYT.ASM")).unwrap();
        let asm = String::from_utf8_lossy(&asm_bytes).to_lowercase();
        for placeholder in ["0c100h", "0c200h", "0c300h", "0c400h"] {
            assert!(asm.contains(placeholder), "ASMYT.ASM carries {placeholder}");
        }
    }

    #[test]
    fn the_plasma_line_is_the_assemblers_index_expression() {
        let t = tables();
        // Row 7 with the startup parameters (PLZ.C:49-50).
        let out = plasma_line(&t, 7, 3500, 2300, 3900, 3670);
        assert_eq!(out.len(), 84);
        // Independent evaluation of the two lookups for a handful of bytes.
        for &r in &[0usize, 1, 39, 80, 83] {
            let r_i = r as i32;
            // The word indices the patched displacements give, while they stay
            // inside their own tables.
            let b = t.lsini16[(2300 + 320 - 4 * r_i + 7) as usize] as i32;
            let px = t.psini[(3500 + 8 * r_i + b) as usize];
            let d = t.lsini4[(3670 + 16 * r_i + 7) as usize] as i32;
            let py = t.psini[(3900 + 320 - 4 * r_i + d + 14) as usize];
            assert_eq!(out[r], px.wrapping_add(py), "byte {r}");
            assert_eq!(out[r], t.plasma_value(r_i, 7, 3500, 2300, 3900, 3670));
        }
        // At r = 83 with a small c2, `setplzparas` puts lc2's displacement at
        // 0x7FE4, below `lsini16`: the read lands in `lsini4`'s tail and the
        // original's 16-bit address is what the port follows.
        let wrapped = t.plasma_value(83, 0, 3500, 2, 3900, 3670);
        let address = MAP_LSINI16
            .wrapping_add(4)
            .wrapping_sub(8 * 83)
            .wrapping_add(640);
        assert!(
            address < MAP_LSINI16,
            "the lc2 displacement underflows lsini16"
        );
        assert_eq!(wrapped, plasma_line(&t, 0, 3500, 2, 3900, 3670)[83]);
        // The four parameters reach the expression.
        assert_ne!(out, plasma_line(&t, 7, 1000, 2000, 3000, 4000));
    }

    #[test]
    fn the_timetable_events_happen_at_the_getmframe_thresholds() {
        // The source's loop: a switch when getmframe exceeds the timetable
        // entry, `curpal` incrementing each time. Stop where the tree's exit
        // would fire (four switches), before the dead 2778 row can index past
        // the array.
        let mut ttptr = 0usize;
        let mut curpal = 1i32;
        let mut switches = Vec::new();
        for frame in 0..3000i32 {
            if ttptr >= 4 {
                break;
            }
            if frame > TIMETABLE[ttptr] {
                switches.push((frame, ttptr, curpal));
                curpal += 1;
                ttptr += 1;
            }
        }
        assert_eq!(
            &switches[..4],
            &[(724, 0, 1), (1492, 1, 2), (1876, 2, 3), (2260, 3, 4)]
        );
        // The dead row: the tree's exit (curpal == 5) fires 65 ticks after the
        // fourth switch, long before 2778.
        assert_eq!(switches[3].0 + 65, 2325);
        assert!(switches[3].0 + 65 < TIMETABLE[4]);
        // The release exits one switch earlier: at the third switch + 65.
        assert_eq!(switches[2].0 + 65, 1941);
    }

    #[test]
    fn the_release_exit_index_is_the_measured_one() {
        // The reference's own ticks: setmframe at 5723; the trace's zframe
        // crossings at boots 6698, 7733 and 8250; the 320x400 screen gone by
        // 8340 (reference-indices.md 13.7). The third switch is at part tick
        // 2527 and the exit 65 ticks later is tick 2592 = boot 8315, inside the
        // refused window.
        let switch_ticks = [
            6698 - REFERENCE_FIRST_INDEX,
            7733 - REFERENCE_FIRST_INDEX,
            8250 - REFERENCE_FIRST_INDEX,
        ];
        assert_eq!(switch_ticks, [975, 2010, 2527]);
        assert_eq!(switch_ticks[2] + 65, 2592);
        assert_eq!(REFERENCE_FIRST_INDEX + switch_ticks[2] + 65, 8315);
        let window = 8310..=8340;
        assert!(window.contains(&8315));
        // The tree's fourth switch would be boot 8769, far past the window.
        assert_eq!(8769 - REFERENCE_FIRST_INDEX, 3046);
        assert_eq!(EXIT_PALETTE_RELEASE, 4);
        assert_eq!(EXIT_PALETTE_TREE, 5);
    }

    #[test]
    fn do_drops_table_and_split_walk_are_the_assemblys() {
        let dtau = build_dtau();
        assert_eq!(dtau[0], 60);
        assert_eq!(dtau[2], 60);
        assert_eq!(dtau[40], 194);
        assert_eq!(dtau[64], 404);
        // The split is the register + 1; 404 walks it off the 400-row screen.
        assert_eq!(dtau[64] + 1, 405);
        assert!(dtau[64] + 1 > PLASMA_HEIGHT);
    }

    #[test]
    fn the_split_and_pan_alternate_at_the_reference_parity() {
        // pompota's register sequence with the port's one-retrace phase: odd
        // pompi → 61/no pan, even → 60/pan 4.
        let mut pompi = 1u8;
        let mut registers = Vec::new();
        for _ in 0..4 {
            pompi = pompi.wrapping_add(1);
            if pompi & 1 == 1 {
                registers.push((61u16, 0u8));
            } else {
                registers.push((60u16, 4u8));
            }
        }
        assert_eq!(registers, [(60, 4), (61, 0), (60, 4), (61, 0)]);
        // The engine's split is the register plus one and its pan is the
        // register's `(value & 7) / 2`.
        let view_of = |(register, scrl): (u16, u8)| (register + 1, scrl / 2);
        assert_eq!(view_of(registers[0]), (61, 2));
        assert_eq!(view_of(registers[1]), (62, 0));
        // At boot 6000 (part tick 277) the reference's first content row is 61:
        // the composed tick is one retrace after slot 2, so tick 277 sees the
        // call made with pompi = 278 (even) → register 60 → scanline 61.
        let mut pompi = 1u16;
        let mut last = (60u16, 4u8);
        for _tick in 0..277 {
            pompi += 1;
            last = if pompi & 1 == 1 { (61, 0) } else { (60, 4) };
        }
        assert_eq!(last, (60, 4));
        assert_eq!(last.0 + 1, 61);
    }

    #[test]
    fn getspl_interpolates_the_camera_spline_at_the_keyframes() {
        let t = tables();
        let spline_at = |frames: i32| -> [i32; 8] {
            let position = (4 * 256 + frames * 4) & 0xFFFF;
            let segment = (position >> 8) as usize;
            let f = (position & 255) as usize;
            let word = |index: usize| t.spline[index] as i32;
            let control = |offset: usize| t.rata[segment * 8 + offset] as i32;
            core::array::from_fn(|word_index| {
                let sum = control(24 + word_index) * word(f)
                    + control(16 + word_index) * word(f + 256)
                    + control(8 + word_index) * word(f + 512)
                    + control(word_index) * word(f + 768);
                sum >> 15
            })
        };
        assert_eq!(spline_at(0), [0, 432, 499, 499, 699, 499, 0, 0]);
        assert_eq!(spline_at(136), [0, 0, 499, 709, 489, 289, 9, 0]);
        assert_eq!(spline_at(1136), [0, 0, 299, 677, 522, 322, 255, 0]);
        // The `REPT 100` tail (RATA.INC) holds the camera at dis 500 with all
        // three angles zero and the light at `ls_kx 255`, `ls_ky 511`; a
        // parser that dropped the repeat would index past its 37 points here.
        assert_eq!(spline_at(3000), [0, 0, 499, 0, 0, 0, 255, 511]);
        assert_eq!(spline_at(6000), [0, 0, 499, 0, 0, 0, 255, 511]);
    }

    #[test]
    fn the_cube_projection_is_the_releases_centred_scale() {
        let mut part = Plzpart::new();
        part.cxx = 256; // an 8.8 identity in x
        part.cyy = 256;
        part.czz = 256;
        part.dis = 320;
        part.tx = 0;
        part.ty = -50;
        part.rotate();
        // (125,125,125) with the identity matrix and dis 320:
        // xx = 125, yy = 125, zz = 125+320 = 445.
        assert_eq!(part.xx[0], [125, 75, 445]);
        // The release's projection: x = xx*256/zz + 320 (the tree's literal
        // 160+160), y = yy*213/zz + 100 (measured; see APPROXIMATIONS A23 —
        // the tree's development build compiles 142 and 66).
        assert_eq!(part.xxx[0][0], (125 * 256) / 445 + 160 + 160);
        assert_eq!(part.xxx[0][1], (75 * 213) / 445 + 100);
        assert_eq!(part.xxx[0][0], 391);
        assert_eq!(part.xxx[0][1], 135);
    }

    #[test]
    fn count_const_keeps_the_c_precedence_shifts() {
        let mut part = Plzpart::new();
        part.sinit = vec![0; SINIT_WORDS];
        part.sinit[256] = 32766; // sinit[256], the quarter turn
        part.sinit[512] = 32766; // `kosinit[256] = sinit[256+256]`
        part.kx = 0;
        part.ky = 256;
        part.kz = 256;
        part.count_const();
        // SX = sinit[0] = 0, CX = sinit[256] = 32766.
        // cxx = (CY*CZ) >> (15+7) = 32766^2 >> 22 = 255.
        assert_eq!(part.cxx, 32766i32.wrapping_mul(32766) >> 22);
        assert_eq!(part.cxx, 255);
        // `cxz = -(long)SY >> 7`: -32766 >> 7 = -256.
        assert_eq!(part.cxz, -256);
        // cyx subtracts inside the final shift:
        // ((((SX*CZ+0x4000)>>15)*SY) - (CX*SZ)) >> 22.
        let expected = ((0i32.wrapping_mul(32766).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(32766)
            .wrapping_sub(32766i32.wrapping_mul(32766))
            >> 22;
        assert_eq!(part.cyx, expected);
        // A value where the shift's placement matters: SX = 32766, CX = 0,
        // SZ = 32766, SY = 32766 gives ((32766*32766+0x4000)>>15)*32766 >> 22.
        part.kx = 256;
        part.ky = 256;
        part.kz = 256;
        part.count_const();
        let expected = ((32766i32.wrapping_mul(32766).wrapping_add(0x4000)) >> 15)
            .wrapping_mul(32766)
            .wrapping_sub(32766i32.wrapping_mul(32766))
            >> 22;
        assert_eq!(part.cyx, expected);
        assert_eq!(part.cyx, -1, "the subtraction is inside the shift");
    }

    #[test]
    fn do_clears_mask_underflow_reads_the_byte_before_end_mask() {
        let masks = build_masks();
        // The table's last entry is 1000b, not 1111b: the underflow reads 8
        // where a masked index would read 15 — the preserved bug.
        assert_eq!(masks[END_MASK_AT - 1], 0b1000);
        assert_eq!(masks[END_MASK_AT + 3], 0b1111);
        assert_ne!(masks[END_MASK_AT - 1], masks[END_MASK_AT + 3]);
        let start = [0b1111u8, 0b1110, 0b1100, 0b1000];
        let end = [0b0001u8, 0b0011, 0b0111, 0b1111];
        for i in 0..200 {
            for j in 0..4 {
                assert_eq!(masks[START_MASK_AT + i * 4 + j], start[j]);
                assert_eq!(masks[END_MASK_AT + i * 4 + j], end[j]);
            }
        }
    }

    #[test]
    fn the_cube_beam_wraps_at_sixty_four_kilobytes_of_plane_bytes() {
        let mut part = Plzpart::new();
        part.video = vec![0u8; VIDEO_BYTES];
        part.video[0] = 0x11;
        part.video[(4 * 65_535) as usize] = 0x22;
        let start = 0xAA28u32;
        let row = 137u32;
        assert_eq!(start + row * CUBE_ROW_BYTES as u32, 65_480);
        let pixel = |x: u32| -> u8 {
            let linear = (4 * start + row * 4 * CUBE_ROW_BYTES as u32 + x) & VIDEO_MASK;
            part.video[linear as usize]
        };
        assert_eq!(pixel(4 * (65_535 - 65_480)), 0x22);
        // The row wraps at pixel 224: (65,536-65,480)*4 = 224.
        let linear_at_zero = (4 * start + row * 4 * CUBE_ROW_BYTES as u32 + 224) & VIDEO_MASK;
        assert_eq!(linear_at_zero, 0);
        assert_eq!(pixel(224), 0x11);
        // A 256,000-pixel model would put that pixel at 256,000 instead of 0.
        assert_eq!((4 * 65_536) % 256_000, 6_144);
    }

    #[test]
    fn the_face_palette_is_shaded_at_its_own_block() {
        let mut pal = [0u8; 768];
        pal[0] = 63;
        pal[192] = 32;
        let mut fpal = [0u8; 768];
        shadepal(&mut fpal, &pal, 0, 32);
        // `shadepal` covers 192 bytes: entries 0..63 of the band.
        assert_eq!(fpal[0], ((63u16 * 32) >> 6) as u8);
        assert_eq!(fpal[192], 0, "the same 192-byte block only");
        shadepal(&mut fpal, &pal, 192, 32);
        assert_eq!(fpal[192], ((32u16 * 32) >> 6) as u8);
        assert_eq!(fpal[1], 0);
    }

    #[test]
    fn a_synthetic_square_is_rasterised_where_its_vertices_are() {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        part.kuva1 = vec![7; 16_384];
        part.kuva2 = vec![7; 16_384];
        part.kuva3 = vec![7; 16_384];
        part.dist1 = vec![0; 32_768];
        part.page = 0;
        part.frames = 0;
        part.do_poly([(10, 10), (20, 10), (20, 20), (10, 20)], 0, 0);
        let pixels = |x: usize, y: usize| part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)];
        let mut missed = Vec::new();
        for y in 10..20 {
            for x in 10..20 {
                if pixels(x, y) != 7 {
                    missed.push((x, y));
                }
            }
        }
        assert!(missed.len() < 10, "missed {missed:?}");
        assert_eq!(pixels(0, 0), 0);
        assert_eq!(pixels(300, 100), 0);
    }

    #[test]
    fn a_slanted_quad_is_rasterised_inside_its_edges() {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        part.kuva1 = vec![7; 16_384];
        part.dist1 = vec![0; 32_768];
        part.page = 0;
        part.frames = 0;
        part.do_poly([(313, 63), (330, 54), (308, 168), (273, 198)], 0, 0);
        let pixels = |x: usize, y: usize| part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)];
        let mut minx = 999;
        let mut maxx = 0;
        let mut miny = 999;
        let mut maxy = 0;
        for y in 0..200 {
            for x in 0..320 {
                if pixels(x, y) != 0 {
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        assert!(
            minx >= 270 && maxx <= 335,
            "x within the quad: {minx}..{maxx}"
        );
        assert!(
            miny >= 50 && maxy <= 199,
            "y within the quad: {miny}..{maxy}"
        );
    }

    #[test]
    fn the_sliver_face_stays_inside_its_edges() {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        part.kuva1 = vec![7; 16_384];
        part.dist1 = vec![0; 32_768];
        part.page = 0;
        part.frames = 0;
        part.do_poly([(261, 22), (370, 29), (435, 42), (257, 32)], 0, 0);
        let pixels = |x: usize, y: usize| part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)];
        let mut minx = 999;
        let mut maxx = 0;
        let mut miny = 999;
        let mut maxy = 0;
        for y in 0..200 {
            for x in 0..639 {
                if pixels(x, y) != 0 {
                    minx = minx.min(x);
                    maxx = maxx.max(x);
                    miny = miny.min(y);
                    maxy = maxy.max(y);
                }
            }
        }
        assert!(
            minx >= 255 && maxx <= 437,
            "x within the quad: {minx}..{maxx}"
        );
        assert!(
            miny >= 20 && maxy <= 44,
            "y within the quad: {miny}..{maxy}"
        );
    }

    #[test]
    fn the_texture_runs_left_to_right_across_a_span() {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        // x = 64..190 maps to 0..126; make the texture a row gradient.
        let mut kuva = vec![0u8; 16_384];
        for y in 0..64 {
            for x in 0..256 {
                kuva[y * 256 + x] = x as u8;
            }
        }
        part.kuva1 = kuva;
        part.dist1 = vec![0; 32_768];
        part.page = 0;
        part.frames = 0;
        part.do_poly([(10, 10), (200, 10), (200, 20), (10, 20)], 0, 0);
        let pixels = |x: usize, y: usize| part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)];
        let left = pixels(12, 15);
        let middle = pixels(100, 15);
        let right = pixels(196, 15);
        assert!(
            left < middle && middle < right,
            "the texture x increases to the right"
        );
    }

    #[test]
    fn the_clear_erases_a_moved_quad() {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        part.kuva1 = vec![7; 16_384];
        part.dist1 = vec![0; 32_768];
        part.page = 0;
        part.frames = 0;
        // Four draws: the clear erases the extents three draws back, so the
        // first square's rows go on the fourth draw.
        for y in [10usize, 40, 40, 40] {
            part.do_poly(
                [
                    (10, y as i32),
                    (100, y as i32),
                    (100, y as i32 + 10),
                    (10, y as i32 + 10),
                ],
                0,
                0,
            );
            part.clear();
        }
        let pixels = |x: usize, y: usize| part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)];
        // The first square's rows must be gone; the last one's present.
        let mut stale = 0;
        for y in 10..20 {
            for x in 10..100 {
                if pixels(x, y) != 0 {
                    stale += 1;
                }
            }
        }
        assert_eq!(stale, 0, "the moved square's old rows are not erased");
        assert_ne!(pixels(50, 45), 0, "the current square is drawn");
    }

    #[test]
    fn the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick() {
        // The reference's published row 13 of order 68 first appears at boot
        // 8364 (second3a's trace), i.e. part tick 8364 - 5723 = 2641 (the
        // part's 1-based `tick`; 0-based 2640). The
        // player is seeked to order 62 when the part starts; the published
        // words are eleven polls behind the trace's position, so the entry
        // pre-roll is what puts the gate's first pass at the reference tick.
        // This poll is the measurement, not a preference.
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;
        use sr_engine::music::Music;

        assert_eq!(8364 - REFERENCE_FIRST_INDEX, 2641);
        assert_eq!(CUBE_GATE, 13);
        assert_eq!(CUBE_EXIT_LOW, -4);
        assert_eq!(REFERENCE_FIRST_INDEX, 5723);

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(62);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        // The engine polls once per tick before the part's frame, so this is
        // part tick 0's poll; `PLZ.C:68-69` then calls `setmframe(0)`, which
        // restarts the row clock the published words advance on.
        music.poll();
        music.setmframe(0);
        let mut tick = 0u32;
        while music.musplus() < CUBE_GATE {
            music.poll();
            tick += 1;
            assert!(tick < 3000, "the gate never passes");
        }
        // The part's `tick` is 1-based (the first `frame` call is tick 1), so
        // the 0-based poll counted here is one less: tick 2640 is the frame
        // whose `self.tick` is 2641, boot 5723 + 2641 = 8364.
        assert_eq!(
            tick, 2640,
            "the cube gate passes at 0-based tick {tick}, not the reference's \
             boot 8364 (MUSIC_PRE_ROLL = {MUSIC_PRE_ROLL})"
        );
        assert_eq!(MUSIC_PRE_ROLL, 9);
    }
}
