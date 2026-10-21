//! HARD's first half: the pre-roll, the letterbox and modes 0..4
//! (`HARD/B.C:125-324`, `HARD/BP.C:1-72`).
//!
//! `B.EXE` is a Demo Interrupt Server part (`B.C:131`). Entry clears the
//! screen to index 15, waits for the start cue, wipes a line at row 100 and
//! fills downward, waits 70 retraces, then installs the palette and the
//! letterbox the whole effect lives in: rows 0..34 = 255, row 35 = 254, rows
//! 36..163 = the `A2D0` work band, row 164 = 254, rows 165..199 = 255
//! (`B.C:164-182`). The band is the dot engine's 128-row buffer and is never
//! cleared by the state machine; the part owns every byte of the 320×200
//! screen.
//!
//! The state machine (`B.C:203-324`) runs one loop iteration a tick:
//! `count++`, `setborder(255)`, one `waitb`, the palette uploads, the dots,
//! then `switch(mode)`. Modes 0..4 are ported here — 34 frames of `dots1`,
//! four 73-frame logo phases (`part1init`/`part1`), four one-frame rotations,
//! 42 dot frames and 102 mode-4 frames — and the part retires when mode 4
//! would advance; Task 7 adds modes 5..6 and the full run.
//!
//! **The display phase is measured, not fitted** (D17/D18): a tick's frame is
//! the state the capture shows at `K0 + tick`, `K0 = 356`. The pre-roll is 119
//! ticks — 20 wipe steps, 28 fill iterations, 70 waits and the letterbox —
//! so frame 0 is the first wipe step (16 black pixels at row 100, reference
//! 356) and the letterbox is tick 118 (reference 474). `part1init`'s own
//! `waitb` is a second display tick for mode 1's count 0, which is what puts
//! the first drawn dots at reference 512 and the phase-1 palette at count 50
//! at reference 560 (`reference-indices.md` §24.7).
//!
//! The palette is kept exactly as the source writes it: `pal` is the pending
//! C array, `dac` the mirror of the DAC, and `loadpal(n)` copies only the
//! first `n` bytes. `part1` uploads **96 bytes (32 entries)** (`BP.C:59`);
//! every other upload is 768. Mode 4 builds its moving band and displays the
//! previous frame's band — the source uploads at the top of the iteration,
//! before the switch rebuilds it — which is the measured phase of reference
//! 852's band (`a = 252 - pal1colp` at count 0, not count 1).

use std::path::Path;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::minvball::MsRand;

use super::dots::{add_dot, dodots, dodots2, dots_from_fcrz, initial_dots, Dot, DOT_COUNT, GRID1};

/// The registry name (`parts::registry`).
pub const HARD_NAME: &str = "hard";

/// `FCRZ.U` in the port's curated asset tree (64000 raw bytes, SHA-256
/// `e2c068fe26c00ab3cfc6816e240d1a2670a1ba31bce8ea543861d3c154a1da5c`). The
/// binary's working directory is the repository root; the unit tests, which
/// run from the crate's directory, use `../../assets/...` themselves.
const FCRZ_PATH: &str = "assets/HARD/FCRZ.U";

const SCREEN_WIDTH: usize = 320;
const SCREEN_HEIGHT: usize = 200;
const SCREEN_BYTES: usize = SCREEN_WIDTH * SCREEN_HEIGHT;

/// The visible rows of the `A2D0:0000` work buffer: screen byte `0x2D00`
/// (11520 = row 36) is the band's first byte (`B.C:168-172`).
const BAND_TOP: usize = 36;
const BAND_ROWS: usize = 128;
const BAND_START: usize = BAND_TOP * SCREEN_WIDTH;
const BAND_END: usize = (BAND_TOP + BAND_ROWS) * SCREEN_WIDTH;

/// `loadpal(pal,32*3)` (`BP.C:59`): part 1's upload is 32 entries.
pub const PART1_UPLOAD_BYTES: usize = 32 * 3;
/// Every other upload in the first half.
pub const FULL_UPLOAD_BYTES: usize = 768;

/// One tick of the source's `main` (`B.C:125`), as a display phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// The row-100 wipe: 16 columns a tick, `waitb` every 16 (`B.C:143-147`).
    Wipe { next_x: i16 },
    /// The accelerating fill from row 100 down (`B.C:148-161`).
    Fill { y1: i16, a: i32, b: i32 },
    /// The 70 `dis_waitb` calls (`B.C:162`).
    Wait { left: u8 },
    /// The palette, border and letterbox setup (`B.C:164-182`).
    Letterbox,
    /// The `while(!dis_exit() && mode!=-1)` loop (`B.C:203`).
    Main,
}

/// The part. `docs/fidelity/hard.md` carries the capture and the residuals.
pub struct Hard {
    screen: [u8; SCREEN_BYTES],
    background: [u8; SCREEN_BYTES],
    dots: Vec<Dot>,
    dot_slot: u16,
    /// The four BDO streams, as the bytes `BDO.C` writes.
    streams: [Vec<u8>; 4],
    /// `dotspnt`: which stream the spawners read (0..3 = `dots1..4`).
    dotspnt: usize,
    /// `dotsp`: the stream byte offset, with the source's `+= 4` and reset.
    dotsp: usize,
    /// `static int flag` in `doit1` (`B.C:104`).
    doit1_flag: i32,
    /// `static int gx` in `doit2` (`B.C:115`).
    gx: usize,
    rand: MsRand,
    /// `pal`, `pal1`, `pal2`, `defpal` (`B.C:20-22`, `BP.C:3`) and the DAC.
    pal: [u8; 768],
    pal1: [u8; 768],
    #[allow(dead_code)] // part 3's fade source; Task 7 consumes it
    pal2: [u8; 768],
    defpal: [u8; 768],
    dac: [u8; 768],
    shown: Palette,
    count: i16,
    mode: i16,
    pal1colp: i16,
    phase: Phase,
    /// `part1init`'s internal `waitb` (`BP.C:29`): one extra display tick.
    pause: bool,
}

impl Hard {
    pub fn new() -> Hard {
        Hard {
            screen: [15; SCREEN_BYTES],
            background: [0; SCREEN_BYTES],
            dots: initial_dots(),
            dot_slot: 0,
            streams: std::array::from_fn(|_| Vec::new()),
            dotspnt: 0,
            dotsp: 0,
            doit1_flag: 0,
            gx: 0,
            rand: MsRand::new(),
            pal: [0; 768],
            pal1: [0; 768],
            pal2: [0; 768],
            defpal: [0; 768],
            dac: [0; 768],
            shown: Palette::new(),
            count: -1,
            mode: 0,
            pal1colp: 0,
            phase: Phase::Wipe { next_x: 319 },
            pause: false,
        }
    }

    /// `NEXTMODE` (`B.C:10`): `{ count = -1; mode++; }`.
    fn nextmode(&mut self) {
        self.count = -1;
        self.mode += 1;
    }

    /// `loadpal(pal, cnt)` (`B.C:68-82`): copy the first `cnt` bytes of the
    /// pending palette into the DAC mirror.
    fn loadpal(&mut self, count: usize) {
        self.dac[..count].copy_from_slice(&self.pal[..count]);
    }

    fn band_mut(&mut self) -> &mut [u8] {
        &mut self.screen[BAND_START..BAND_END]
    }

    /// One tick of the row-100 wipe.
    fn wipe_tick(&mut self) {
        let next_x = match self.phase {
            Phase::Wipe { next_x } => next_x,
            _ => unreachable!(),
        };
        for step in 0..16 {
            let x = next_x - step;
            if x >= 0 {
                self.screen[100 * SCREEN_WIDTH + x as usize] = 0;
            }
        }
        let next = next_x - 16;
        self.phase = if next < 0 {
            Phase::Fill {
                y1: 100,
                a: 100 * 64,
                b: 0,
            }
        } else {
            Phase::Wipe { next_x: next }
        };
    }

    /// One tick of the fill loop: `b += 16; a += b; y2 = a/64`, clear rows
    /// `y1..y2` (`B.C:148-161`), then the `waitb` that ends the tick.
    fn fill_tick(&mut self) {
        let (mut y1, mut a, mut b) = match self.phase {
            Phase::Fill { y1, a, b } => (y1, a, b),
            _ => unreachable!(),
        };
        b += 16;
        a += b;
        let mut y2 = a / 64;
        if y2 > 200 {
            y2 = 200;
        }
        for y in y1..y2 as i16 {
            let at = y as usize * SCREEN_WIDTH;
            self.screen[at..at + SCREEN_WIDTH].fill(0);
        }
        y1 = y2 as i16;
        self.phase = if y1 >= 200 {
            Phase::Wait { left: 70 }
        } else {
            Phase::Fill { y1, a, b }
        };
    }

    /// One of the 70 waits (`B.C:162`).
    fn wait_tick(&mut self) {
        let left = match self.phase {
            Phase::Wait { left } => left,
            _ => unreachable!(),
        };
        self.phase = if left > 1 {
            Phase::Wait { left: left - 1 }
        } else {
            Phase::Letterbox
        };
    }

    /// The setup between the waits and the loop (`B.C:164-201`): zero the
    /// palette, upload it, write the border, the five letterbox bands, the
    /// three `defpal` entries, upload again, and prepare the band.
    fn letterbox_tick(&mut self) {
        self.defpal = [0; 768];
        self.pal.copy_from_slice(&self.defpal);
        self.loadpal(FULL_UPLOAD_BYTES);
        // `outp(0x3c0,0x11+0x20); outp(0x3c0,255)` sets the overscan black;
        // `setborder` is inert because `border == 0` (`B.C:36-49,196,201`).
        self.screen[0..35 * SCREEN_WIDTH].fill(255);
        self.screen[35 * SCREEN_WIDTH..36 * SCREEN_WIDTH].fill(254);
        self.screen[BAND_START..BAND_END].fill(0);
        self.screen[164 * SCREEN_WIDTH..165 * SCREEN_WIDTH].fill(254);
        self.screen[165 * SCREEN_WIDTH..SCREEN_BYTES].fill(255);
        self.defpal[0..3].copy_from_slice(&[0, 0, 20]);
        self.defpal[254 * 3..255 * 3].copy_from_slice(&[45, 45, 45]);
        self.defpal[255 * 3..256 * 3].copy_from_slice(&[0, 0, 0]);
        self.pal.copy_from_slice(&self.defpal);
        self.loadpal(FULL_UPLOAD_BYTES);
        self.background.fill(0);
        self.dotspnt = 0; // `dotspnt = dots1` (`B.C:186`)
        self.dotsp = 0;
        // The argument path is not ported (`B.C:187-197`); its debug strip is
        // overwritten at `:200-201` with 255, which rows 0/1 already are.
        self.screen[0..256].fill(255);
        self.screen[SCREEN_WIDTH..SCREEN_WIDTH + 256].fill(255);
        self.shown = palette_from_mirror(&self.dac);
        self.phase = Phase::Main;
    }

    /// `part1init` (`BP.C:6-33`).
    fn part1init(&mut self) {
        if self.pal1colp == 0 {
            self.pal.copy_from_slice(&self.defpal);
        }
        part1init_palette(&mut self.pal1);
        // `waitb(); inp(0x3da);` (`BP.C:29-30`): the extra display tick.
        self.pause = true;
        self.dotsp = 0;
        self.pal1colp += 4;
    }

    /// `part1` (`BP.C:35-72`).
    fn part1(&mut self) {
        part1_palette(&mut self.pal, &self.pal1, self.pal1colp, self.count);
        self.loadpal(PART1_UPLOAD_BYTES);
        if self.count < 8 {
            for _ in 0..DOT_COUNT / 8 {
                self.spawn_part1();
            }
        } else if self.count >= 72 {
            self.nextmode();
        }
    }

    /// One iteration of `part1`'s spawner loop: `dotsp += 4` first, so record
    /// 0 is never read, and a terminator resets `dotsp` to 0 (`BP.C:60-69`).
    fn spawn_part1(&mut self) {
        self.dotsp += 4;
        let (value, x, y) = self.read_stream();
        let pal1colp = self.pal1colp;
        match value {
            1 => self.doit1(x, y, 3 + pal1colp),
            2 => self.doit1(x, y, 2 + pal1colp),
            3 => self.doit1(x, y, 1 + pal1colp),
            _ => self.dotsp = 0,
        }
    }

    fn read_stream(&self) -> (u8, i16, i16) {
        let stream = &self.streams[self.dotspnt];
        let at = self.dotsp;
        (
            stream.get(at).copied().unwrap_or(0),
            stream.get(at + 2).copied().unwrap_or(0) as i8 as i16,
            stream.get(at + 3).copied().unwrap_or(0) as i8 as i16,
        )
    }

    /// `doit1` (`B.C:100-110`).
    fn doit1(&mut self, x: i16, y: i16, colour: i16) {
        self.doit1_flag ^= 1;
        let (mut x2, mut y2) = if self.doit1_flag & 1 != 0 {
            (x.wrapping_mul(4), y.wrapping_mul(4))
        } else {
            (x.wrapping_mul(3), y.wrapping_mul(3))
        };
        x2 = x2.wrapping_add((self.rand.next() & 31) as i16 - 15);
        y2 = y2.wrapping_add((self.rand.next() & 31) as i16 - 15);
        add_dot(
            &mut self.dots,
            x.wrapping_add(160),
            y.wrapping_add(64),
            x2.wrapping_add(160),
            y2.wrapping_add(64),
            colour as u8,
            &mut self.dot_slot,
        );
    }

    /// `doit2` (`B.C:112-121`): the grid table supplies the spawn centre and
    /// the logo offset the initial position.
    fn doit2(&mut self, x: i16, y: i16, colour: u8) {
        let x2 = GRID1[self.gx];
        let y2 = GRID1[self.gx + 1];
        self.gx += 2;
        if self.gx >= 200 {
            self.gx = 0;
        }
        add_dot(
            &mut self.dots,
            x2,
            y2,
            x.wrapping_add(160),
            y.wrapping_add(64),
            colour,
            &mut self.dot_slot,
        );
    }

    /// `adddot(0,0,0,0,0)` 2,048 times: mark every record dead.
    fn kill_all(&mut self) {
        for _ in 0..DOT_COUNT {
            add_dot(&mut self.dots, 0, 0, 0, 0, 0, &mut self.dot_slot);
        }
    }

    /// Mode 4's `if(!count)` block (`B.C:247-268`): reset `pal1colp`, rebuild
    /// the background from the last logo stream with colour 254, and set the
    /// band's base entries.
    fn mode_four_rebuild(&mut self) {
        self.pal1colp = 1;
        self.background.fill(0);
        self.dotsp = 0;
        for _ in 0..DOT_COUNT {
            self.dotsp += 4;
            let (value, x, y) = self.read_stream();
            match value {
                1..=3 => self.doit2(x, y, 254),
                _ => self.dotsp = 0,
            }
        }
        mode_four_init_palette(&mut self.pal);
    }

    /// One main-loop iteration (`B.C:203-324`). Returns true when the part
    /// retires: mode 4 would advance, which is this task's boundary.
    fn main_tick(&mut self) -> bool {
        if self.pause {
            self.pause = false;
            return false;
        }
        self.count += 1;
        // `setborder(255)` and `waitb` (`B.C:206-207`): the border variable is
        // 0, so `setborder` returns at `B.C:38`, and one wait is one tick.
        if self.mode == 4 && self.count > 0 {
            // `loadpal(pal,768)` (`B.C:208`): the band built last iteration.
            self.loadpal(FULL_UPLOAD_BYTES);
        }
        // `setborder(0)` (`B.C:211`).
        if self.mode < 4 {
            dodots(
                &mut self.dots,
                &mut self.background,
                &mut self.screen[BAND_START..BAND_END],
            );
        } else if self.mode == 4 {
            dodots2(&mut self.dots, &mut self.screen[BAND_START..BAND_END]);
        }
        match self.mode {
            0 => {
                if self.count > 32 {
                    self.nextmode();
                }
            }
            1 => {
                if self.count == 0 {
                    self.part1init();
                } else {
                    self.part1();
                }
            }
            2 => {
                self.nextmode();
                match self.dotspnt {
                    0 => {
                        self.mode = 1;
                        self.dotspnt = 1;
                    }
                    1 => {
                        self.mode = 1;
                        self.dotspnt = 2;
                    }
                    2 => {
                        self.mode = 1;
                        self.dotspnt = 3;
                    }
                    _ => self.kill_all(),
                }
            }
            3 => {
                if self.count > 40 {
                    self.nextmode();
                }
            }
            4 => {
                // The `count>400` branch (`B.C:243-246`) is dead: the
                // `count>100` branch below advances at 101.
                if self.count > 400 {
                    self.nextmode();
                }
                if self.count == 0 {
                    self.mode_four_rebuild();
                }
                mode_four_band(&mut self.pal, self.pal1colp);
                self.pal1colp += 1;
                if self.count > 100 {
                    self.kill_all();
                    self.nextmode();
                }
            }
            _ => {
                // Modes 5..6 and the final `default` are Task 7's.
            }
        }
        self.shown = palette_from_mirror(&self.dac);
        // The mode-4 advance tick is this task's boundary: mode 4's
        // `count>100` branch (`B.C:288-292`) has killed the dots and run
        // `NEXTMODE`, and Task 7 replaces this return with modes 5..6.
        self.mode >= 5
    }
}

impl Part for Hard {
    fn name(&self) -> &'static str {
        HARD_NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let raw = std::fs::read(Path::new(FCRZ_PATH))
            .or_else(|_| std::fs::read(Path::new("../../").join(FCRZ_PATH)))
            .map_err(|error| PartError {
                part: HARD_NAME,
                message: format!("{FCRZ_PATH}: {error}"),
            })?;
        if raw.len() != SCREEN_BYTES {
            return Err(PartError {
                part: HARD_NAME,
                message: format!("{FCRZ_PATH}: {} bytes, expected 64000", raw.len()),
            });
        }
        let mut image = [0u8; SCREEN_BYTES];
        image.copy_from_slice(&raw);
        self.streams = dots_from_fcrz(&image);
        self.dots = initial_dots();
        self.dot_slot = 0;
        self.rand = MsRand::new();
        // No pre-roll: the vendored `B.C` burns no `rand()` calls before its
        // first spawn, and the capture says the artifact agrees. The plan's
        // and the research's "60-call pre-roll" (`B.C:122`) does not exist in
        // the tree; with the burn the first drawn frame is a different 92
        // pixels from reference 512's 93, without it the 93 are exact
        // (`docs/fidelity/hard.md`, `APPROXIMATIONS.md` A46).
        // `int 10h` mode 13h, `waitb()`, `memset(vram0,15,64000)` (`B.C:133-137`).
        self.screen = [15; SCREEN_BYTES];
        self.phase = Phase::Wipe { next_x: 319 };
        self.count = -1;
        self.mode = 0;
        self.pal1colp = 0;
        self.pause = false;
        self.dotspnt = 0;
        self.dotsp = 0;
        self.doit1_flag = 0;
        self.gx = 0;
        self.pal = [0; 768];
        self.pal1 = [0; 768];
        self.pal2 = [0; 768];
        self.defpal = [0; 768];
        self.dac = [0; 768];
        // The mode set leaves this DAC in place until the pre-roll's first
        // `loadpal(defpal,768)`.
        self.shown = crate::shell::hardware_default_palette();
        ctx.view.width = SCREEN_WIDTH as u16;
        ctx.view.height = SCREEN_HEIGHT as u16;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(SCREEN_WIDTH as u16);
        ctx.view.set_origin(0);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
        ctx.set_palette(self.shown);
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `while(!dis_exit() && mode!=-1)` (`B.C:203`); the pre-roll's wipe
        // and fill do not poll, the 70 waits do (`B.C:162`). The port ends on
        // the exit tick wherever it is seen, as the other parts do.
        if ctx.exit_requested {
            return PartStatus::Finished;
        }
        let finished = match self.phase {
            Phase::Wipe { .. } => {
                self.wipe_tick();
                false
            }
            Phase::Fill { .. } => {
                self.fill_tick();
                false
            }
            Phase::Wait { .. } => {
                self.wait_tick();
                false
            }
            Phase::Letterbox => {
                self.letterbox_tick();
                false
            }
            Phase::Main => self.main_tick(),
        };
        ctx.surface.pixels_mut()[..SCREEN_BYTES].copy_from_slice(&self.screen);
        ctx.set_palette(self.shown);
        if finished {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }

    fn band_count(&self) -> u16 {
        1
    }
}

// ------------------------------------------------------------- palettes --

/// A DAC mirror as an engine [`Palette`]. The bytes are already 6-bit.
fn palette_from_mirror(bytes: &[u8; 768]) -> Palette {
    let mut palette = Palette::new();
    for index in 0..256 {
        let at = index * 3;
        palette.set_entry(
            index as u8,
            [bytes[at] & 63, bytes[at + 1] & 63, bytes[at + 2] & 63],
        );
    }
    palette
}

/// `part1init`'s `pal1` build (`BP.C:10-28`), byte for byte.
///
/// For `a = 0..63`: `b = (64-a)^2 / 64` clamped to 0..63 goes to the red and
/// green planes of the three groups at byte offsets `48`, `48+192` and
/// `48+384`, scaled by 1, 2/3 and 1/3; the blue plane is `(64-a)*4/6`
/// clamped, scaled the same way and biased by 20.
pub fn part1init_palette(pal1: &mut [u8; 768]) {
    for a in 0..64usize {
        let mut b = 64 - a as i32;
        b = b * b / 64;
        b = b.clamp(0, 63);
        pal1[a * 3 + 48] = b as u8;
        pal1[a * 3 + 48 + 64 * 3] = (b * 2 / 3) as u8;
        pal1[a * 3 + 48 + 128 * 3] = (b / 3) as u8;
        pal1[a * 3 + 48 + 1] = b as u8;
        pal1[a * 3 + 48 + 64 * 3 + 1] = (b * 2 / 3) as u8;
        pal1[a * 3 + 48 + 128 * 3 + 1] = (b / 3) as u8;
        let mut blue = (64 - a as i32) * 4 / 6;
        blue = blue.clamp(0, 63);
        pal1[a * 3 + 48 + 2] = (blue + 20) as u8;
        pal1[a * 3 + 48 + 64 * 3 + 2] = (blue * 2 / 3 + 20) as u8;
        pal1[a * 3 + 48 + 128 * 3 + 2] = (blue / 3 + 20) as u8;
    }
}

/// `part1`'s per-frame palette writes (`BP.C:38-58`), at the source's literal
/// byte offsets.
///
/// `pal1colp` is 4, 8, 12, 16 across the four phases, so the two groups land
/// at entries 1..3/5..7, 5..7/9..11, 9..11/13..15 and 13..15/17..19. Every
/// byte written is below 96, which is all `loadpal(pal,32*3)` uploads.
pub fn part1_palette(pal: &mut [u8; 768], pal1: &[u8; 768], pal1colp: i16, count: i16) {
    let first = (count - 6).clamp(0, 63) as usize * 3 + 16 * 3;
    let second = (64 - count.min(64)) as usize * 3 + 16 * 3;
    let base = pal1colp as usize * 3;
    let first_group = base.wrapping_sub(3 * 4);
    for (group, offset) in [(0usize, 3usize), (64, 6), (128, 9)] {
        for channel in 0..3 {
            pal[first_group + offset + channel] = pal1[group * 3 + first + channel];
            pal[base + offset + channel] = pal1[group * 3 + second + channel];
        }
    }
}

/// Mode 4's `if(!count)` palette writes (`B.C:262-267`): entries 1..31 are
/// `(0,0,20)` before the band is built.
pub fn mode_four_init_palette(pal: &mut [u8; 768]) {
    for at in (3..32 * 3).step_by(3) {
        pal[at..at + 3].copy_from_slice(&[0, 0, 20]);
    }
}

/// Mode 4's moving band (`B.C:269-286`).
///
/// `a = 252 - pal1colp`, 66 entries (clamped at 254), `c` from 63 down by 4
/// with the source's `c<0 -> 0`, `b = c*c/64`, `(b, b, max(b,20))`; then
/// entries **189** and **1** are forced to 63.
///
/// The plan's and the research's summary say "pal[251] = pal[3] = 63"; the
/// tree says `a = 254-65 = 189` and `pal[1*3]`, and the capture agrees: mode
/// 5's rebuild tests `vram[u] == 254-65` (`B.C:299`), and the held mode-4
/// trail is index 189 (`docs/fidelity/hard.md`).
pub fn mode_four_band(pal: &mut [u8; 768], pal1colp: i16) {
    let mut a = 252 - pal1colp as i32;
    let mut d = a + 66;
    if d > 254 {
        d = 254;
    }
    let mut c = 63i32;
    while a < d {
        let level = c.max(0);
        let b = (level * level / 64) as u8;
        let at = a as usize * 3;
        pal[at] = b;
        pal[at + 1] = b;
        pal[at + 2] = b.max(20);
        a += 1;
        c -= 4;
    }
    let at = (254 - 65) * 3;
    pal[at..at + 3].copy_from_slice(&[63, 63, 63]);
    pal[3..6].copy_from_slice(&[63, 63, 63]);
}

#[cfg(test)]
mod tests {
    use sr_engine::{Engine, IndexedFrame, View};

    use super::*;

    fn start() -> (Engine, Hard, IndexedFrame) {
        let mut engine = Engine::new(320, 200, View::new(320, 200));
        let mut part = Hard::new();
        engine.start(&mut part).expect("HARD starts");
        let frame = IndexedFrame::new(320, 200);
        (engine, part, frame)
    }

    fn tick(engine: &mut Engine, part: &mut Hard, frame: &mut IndexedFrame) -> PartStatus {
        engine.tick(part, frame)
    }

    #[test]
    fn the_pre_roll_is_the_sources_tick_counts() {
        let (mut engine, mut part, mut frame) = start();
        assert_eq!(part.phase, Phase::Wipe { next_x: 319 });
        // Frame 0: the first 16 columns of row 100 are black; every other
        // pixel is the white page (`B.C:137,143-147`).
        assert_eq!(
            tick(&mut engine, &mut part, &mut frame),
            PartStatus::Continuing
        );
        for x in 0..320 {
            let expected = if x >= 304 { 0 } else { 15 };
            assert_eq!(frame.row(100)[x], expected, "x {x}");
        }
        assert_eq!(frame.row(99), &[15u8; 320]);
        assert_eq!(frame.row(101), &[15u8; 320]);

        // 20 wipe steps: 356..375 in the capture, the whole row black at the
        // 20th.
        let mut wipe_ticks = 1;
        while matches!(part.phase, Phase::Wipe { .. }) {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
            wipe_ticks += 1;
        }
        assert_eq!(wipe_ticks, 20);
        assert_eq!(frame.row(100), &[0u8; 320]);
        assert_eq!(frame.row(99), &[15u8; 320]);

        // 28 fill iterations: 376..403. The first two clear no new row, the
        // third clears the already-black row 100; iteration 4 leaves rows
        // 100..101 black (`reference-indices.md` §24.7's 376-380).
        let mut fill_ticks = 0;
        while matches!(part.phase, Phase::Fill { .. }) {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
            fill_ticks += 1;
            if fill_ticks == 4 {
                assert_eq!(frame.row(100), &[0u8; 320]);
                assert_eq!(frame.row(101), &[0u8; 320]);
                assert_eq!(frame.row(102), &[15u8; 320]);
            }
        }
        assert_eq!(fill_ticks, 28);
        assert_eq!(frame.row(199), &[0u8; 320]);

        // 70 waits: 404..473, no change.
        let mut wait_ticks = 0;
        while matches!(part.phase, Phase::Wait { .. }) {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
            wait_ticks += 1;
        }
        assert_eq!(wait_ticks, 70);
        assert_eq!(frame.row(199), &[0u8; 320]);

        // The letterbox tick is reference 474 and starts the loop at 475.
        assert_eq!(part.phase, Phase::Letterbox);
        assert_eq!(
            tick(&mut engine, &mut part, &mut frame),
            PartStatus::Continuing
        );
        assert_eq!(part.phase, Phase::Main);
        // 119 pre-roll ticks: reference 474 = K0 + 118.
        assert!(frame.row(0).iter().all(|&p| p == 255));
    }

    #[test]
    fn the_letterbox_and_defpal_are_the_sources() {
        let (mut engine, mut part, mut frame) = start();
        for _ in 0..119 {
            tick(&mut engine, &mut part, &mut frame);
        }
        assert_eq!(part.phase, Phase::Main);
        // The five bands (`B.C:168-172`).
        for y in 0..35 {
            assert!(frame.row(y as u16).iter().all(|&p| p == 255), "row {y}");
        }
        assert!(frame.row(35).iter().all(|&p| p == 254));
        for y in 36..164 {
            assert!(frame.row(y).iter().all(|&p| p == 0), "row {y}");
        }
        assert!(frame.row(164).iter().all(|&p| p == 254));
        for y in 165..200 {
            assert!(frame.row(y).iter().all(|&p| p == 255), "row {y}");
        }
        // `defpal[0] = (0,0,20)`, `[254] = (45,45,45)`, `[255] = (0,0,0)`
        // (`B.C:173-182`), everything else zero.
        assert_eq!(&part.dac[0..3], &[0, 0, 20]);
        assert_eq!(&part.dac[254 * 3..255 * 3], &[45, 45, 45]);
        assert_eq!(&part.dac[255 * 3..256 * 3], &[0, 0, 0]);
        for index in 1..254 {
            assert_eq!(
                &part.dac[index * 3..index * 3 + 3],
                &[0, 0, 0],
                "entry {index}"
            );
        }
        // The view is the whole mode-13h screen at origin 0, stride 320.
        assert_eq!(engine.view().y_line_offset, Some(320));
        assert_eq!((engine.view().width, engine.view().height), (320, 200));
        assert!(!engine.view().double_scan);
        assert!(engine.view().split.is_none());
    }

    #[test]
    fn the_part1_palette_uploads_only_32_entries() {
        let mut pal1 = [0u8; 768];
        part1init_palette(&mut pal1);
        // The capture's measured phase-1 colour at reference 560 (count 50):
        // the second group's entry 5 is (39,39,53). The group's own a is
        // 64 - 50 = 14, so b = (64-14)^2/64 = 39 and blue = 50*4/6 + 20 = 53.
        let mut pal = [0u8; 768];
        part1_palette(&mut pal, &pal1, 4, 50);
        assert_eq!(&pal[5 * 3..5 * 3 + 3], &[39, 39, 53]);
        assert_eq!(&pal[6 * 3..6 * 3 + 3], &[26, 26, 42]);
        assert_eq!(&pal[7 * 3..7 * 3 + 3], &[13, 13, 31]);
        // The first group at count 50 reads pal1 entry 44.
        assert_eq!(&pal[3..6], &[6, 6, 33]);
        assert_eq!(&pal[2 * 3..2 * 3 + 3], &[4, 4, 28]);
        assert_eq!(&pal[3 * 3..3 * 3 + 3], &[2, 2, 24]);

        // Every byte the frame writes is inside the 96-byte upload, for every
        // count and phase.
        for pal1colp in [4i16, 8, 12, 16] {
            for count in 0..=72i16 {
                let mut written = [false; 768];
                let mut scratch = [0u8; 768];
                let before = scratch;
                part1_palette(&mut scratch, &pal1, pal1colp, count);
                for (at, (old, new)) in before.iter().zip(&scratch).enumerate() {
                    if old != new {
                        written[at] = true;
                    }
                }
                let highest = written.iter().rposition(|&w| w);
                assert!(
                    highest.is_none_or(|at| at < PART1_UPLOAD_BYTES),
                    "pal1colp {pal1colp}, count {count}: byte {highest:?}"
                );
            }
        }
        assert_eq!(PART1_UPLOAD_BYTES, 96);
        assert_eq!(FULL_UPLOAD_BYTES, 768);
    }

    #[test]
    fn the_mode_four_band_palette_is_the_sources() {
        let mut pal = [0u8; 768];
        mode_four_init_palette(&mut pal);
        for at in (3..32 * 3).step_by(3) {
            assert_eq!(&pal[at..at + 3], &[0, 0, 20]);
        }
        // Count 0's band: a = 251..253, c = 63, 59, 55.
        mode_four_band(&mut pal, 1);
        assert_eq!(&pal[251 * 3..251 * 3 + 3], &[62, 62, 62]);
        assert_eq!(&pal[252 * 3..252 * 3 + 3], &[54, 54, 54]);
        assert_eq!(&pal[253 * 3..253 * 3 + 3], &[47, 47, 47]);
        // The two forced entries (`B.C:280-286`; the plan's 251/3 is the
        // research's transcription error — the capture's mode-5 test at
        // `B.C:299` is `254-65`).
        assert_eq!(&pal[189 * 3..189 * 3 + 3], &[63, 63, 63]);
        assert_eq!(&pal[3..6], &[63, 63, 63]);

        // A later band: a = 187, 66 entries clamped at 254; the loop's value
        // at 189 is overwritten by the forced white.
        let mut pal = [0u8; 768];
        mode_four_band(&mut pal, 65);
        assert_eq!(&pal[187 * 3..187 * 3 + 3], &[62, 62, 62]);
        assert_eq!(&pal[188 * 3..188 * 3 + 3], &[54, 54, 54]);
        assert_eq!(&pal[189 * 3..189 * 3 + 3], &[63, 63, 63]);
        assert_eq!(&pal[190 * 3..190 * 3 + 3], &[40, 40, 40]);
        assert_eq!(&pal[252 * 3..252 * 3 + 3], &[0, 0, 20]);
        assert_eq!(&pal[253 * 3..253 * 3 + 3], &[0, 0, 0]);
    }

    #[test]
    fn the_first_half_mode_sequence_is_the_sources() {
        let (mut engine, mut part, mut frame) = start();
        // The pre-roll, then walk the modes and note each transition.
        let mut seen: Vec<(u32, i16, i16)> = Vec::new();
        let mut previous = (part.mode, part.count);
        let mut finished_at = None;
        for part_frame in 0..700u32 {
            let status = tick(&mut engine, &mut part, &mut frame);
            if (part.mode, part.count) != previous {
                seen.push((part_frame, part.mode, part.count));
                previous = (part.mode, part.count);
            }
            if status == PartStatus::Finished {
                finished_at = Some(part_frame);
                break;
            }
        }
        let finished_at = finished_at.expect("the first half ends at the mode-5 boundary");
        // 119 pre-roll ticks, then mode 0's 34 iterations (counts 0..33),
        // part1's four 73-frame phases (counts 0..72) with four one-frame
        // rotations, mode 3's 42 frames and mode 4's 102; the count-0 of each
        // part-1 phase is two display ticks (`part1init`'s own waitb).
        let mode_counts: Vec<i16> = seen
            .iter()
            .filter(|(_, mode, count)| *mode == 4 && *count == 0)
            .map(|(_, _, count)| *count)
            .collect();
        assert_eq!(mode_counts.len(), 1);
        let mode_four_start = seen
            .iter()
            .find(|(_, mode, count)| *mode == 4 && *count == 0)
            .map(|(frame, _, _)| *frame)
            .unwrap();
        // 119 pre-roll ticks, mode 0's 34, the four phases' 4*73 frames and
        // their 4 rotation frames, and mode 3's 42.
        assert_eq!(mode_four_start, 119 + 34 + 4 * (73 + 1) + 4 + 42);
        assert_eq!(
            finished_at,
            mode_four_start + 101,
            "mode 4 runs counts 0..101 and the part retires on the last"
        );

        // The mode-2 rotations are one frame each: after four of them the
        // stream index has walked dots1 -> dots4 and the dots are dead.
        assert_eq!(part.dotspnt, 3);
        assert!(part.dots.iter().all(|dot| dot.colour == 0));

        // `part1`'s spawner skips record 0 of each stream and spawns 256
        // dots on each of counts 1..7 of a phase; `part1init` spawns none.
        // At the end of phase 4 the slot has advanced 4 * 7 * 256, plus
        // mode 4's 2048 rebuild and the 2048 kill.
        // The spawner's 256-iteration budget is not 256 spawns: every time a
        // stream's terminator is read that iteration only resets `dotsp`
        // (`BP.C:68`). A stream of R records yields `N - N/R` spawns from N
        // iterations. The four phases spawn from `dots1..4`; the mode-2
        // fourth rotation kills 2,048 records, mode 4's rebuild spawns from
        // `dots4` (2,047 of its 2,048 iterations) and its advance kills
        // 2,048.
        let spawns = |records: usize, iterations: usize| iterations - iterations / records;
        let records: Vec<usize> = part.streams.iter().map(|s| (s.len() - 1) / 4).collect();
        assert_eq!(records, [859, 690, 722, 1_071]);
        let adds = records[0..3]
            .iter()
            .map(|&r| spawns(r, 1_792))
            .sum::<usize>()
            + spawns(records[3], 1_792)
            + 2_048
            + spawns(records[3], 2_048)
            + 2_048;
        assert_eq!(part.dot_slot, (adds % 2_048) as u16);
    }

    #[test]
    fn the_first_spawn_is_the_sources_rand_sequence() {
        let (mut engine, mut part, mut frame) = start();
        // Run to the first phase's count 1: 119 pre-roll, 34 mode-0 and
        // part1init's two ticks = 155; tick 155 is count 1.
        for _ in 0..156 {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
        }
        assert_eq!((part.mode, part.count), (1, 1));
        assert_eq!(part.dot_slot, 256, "2048/8 spawns on count 1");

        // The first spawned dot is record 1 of `dots1` (record 0 is skipped
        // by `dotsp += 4`), scaled by the first `doit1`'s x4 and jittered by
        // the first two `rand()` calls of the Microsoft C 6.0 LCG. Recompute
        // it from the LCG and the source's `adddot` arithmetic.
        let raw = std::fs::read("../../assets/HARD/FCRZ.U").unwrap();
        let mut image = [0u8; 64_000];
        image.copy_from_slice(&raw);
        let streams = dots_from_fcrz(&image);
        let stream = &streams[0];
        let value = stream[4];
        assert!((1..=3).contains(&value));
        let x = stream[6] as i8 as i16;
        let y = stream[7] as i8 as i16;
        let mut seed = 1u32;
        let mut rand = || {
            seed = seed.wrapping_mul(214_013).wrapping_add(2_531_011);
            ((seed >> 16) & 0x7FFF) as i32
        };
        let x2 = x.wrapping_mul(4).wrapping_add((rand() & 31) as i16 - 15);
        let y2 = y.wrapping_mul(4).wrapping_add((rand() & 31) as i16 - 15);
        let expected = Dot {
            x: (x2.wrapping_add(160))
                .wrapping_mul(64)
                .wrapping_add(32)
                .wrapping_sub((x.wrapping_add(160)).wrapping_sub(x2.wrapping_add(160))),
            y: (y2.wrapping_add(64))
                .wrapping_mul(64)
                .wrapping_add(32)
                .wrapping_sub((y.wrapping_add(64)).wrapping_sub(y2.wrapping_add(64))),
            vx: x.wrapping_add(160).wrapping_sub(x2.wrapping_add(160)),
            vy: y.wrapping_add(64).wrapping_sub(y2.wrapping_add(64)),
            life: 66,
            colour: (3 + part.pal1colp) as u8,
            last: (x2.wrapping_add(160) as u16)
                .wrapping_add((y2.wrapping_add(64) as u16).wrapping_mul(320)),
        };
        assert_eq!(part.dots[0], expected);
        // And the array's rest is dead: only the 256 slots just written are
        // live, the other 1792 keep the initial `0x100`.
        assert_eq!(part.dots.iter().filter(|d| d.colour != 0).count(), 256);
    }
}
