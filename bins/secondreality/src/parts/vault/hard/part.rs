//! HARD: the pre-roll, the letterbox, the logo dots and the two 3-D phases
//! (`HARD/B.C:125-324`, `HARD/BP.C:1-299`).
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
//! then `switch(mode)`. Modes 0..4 are 34 frames of `dots1`, four 73-frame
//! logo phases (`part1init`/`part1`), four one-frame rotations, 42 dot frames
//! and 102 mode-4 frames. Mode 5 is part 2 (`BP.C:124-215`): a 488-point
//! 10×10×10 lattice shell projected through `crotprojlist` and drawn by
//! `do3dots`, 500 frames of it, dimmed by 255/256 over the last sixteen (the
//! source's negative-`a` fade; `docs/fidelity/hard.md`). Mode 6 is
//! part 3 (`BP.C:220-299`): a 128-row glenz cube drawn twice through the
//! `clipz = 4000` clipper — a magnified copy with `ceasymode(0)`/`demo_hide`
//! and the cube itself with `ceasymode(1)`/`demo_norm` — 800 frames, the
//! palette ramped in over the first twenty. Mode 7 and the `default` arm
//! retire the part (`B.C:313-323`). The band is **not cleared** between
//! part-3 frames (`VNEW.ASM:27-56`); the compositor is seeded from the A2D0
//! band part 2 left.
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


use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::glenz::math::{matrix_yxz, Matrix};
use crate::parts::sin1024::SIN1024;
use crate::parts::msrand::MsRand;

use super::dots::{
    add_dot, depthcol_table, do3dots, dodots, dodots2, dots_from_fcrz, grid_points, initial_dots,
    Dot, DOT_COUNT, GRID1,
};
use super::math::{project, rotate, rotate_project, PVert};
use super::poly::{clip_easy_list, FaceMode, Poly, PolyDef};
use super::span::Compositor;

use crate::parts::{mode_view, read_asset};

/// The registry name (`parts::registry`).
pub const HARD_NAME: &str = "hard";

/// `FCRZ.U` in the port's curated asset tree (64000 raw bytes, SHA-256
/// `e2c068fe26c00ab3cfc6816e240d1a2670a1ba31bce8ea543861d3c154a1da5c`).
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

/// The `A2D0` band's byte count: `128*320` (`B.C:297`). The compositor's
/// 64,000-byte work buffer holds 200 rows, but `ng_pass2` paints 128
/// (`VNEW.ASM:4,162`) and only those are displayed.
const BAND_BYTES: usize = BAND_END - BAND_START;

/// `loadpal(pal,32*3)` (`BP.C:59`): part 1's upload is 32 entries.
pub const PART1_UPLOAD_BYTES: usize = 32 * 3;
/// Every other upload in the first half.
pub const FULL_UPLOAD_BYTES: usize = 768;

/// `if(count>=500) NEXTMODE` (`BP.C:172`): part 2's end.
const PART2_END: i16 = 500;
/// `if(count>=800) NEXTMODE` (`BP.C:262`): part 3's end.
const PART3_END: i16 = 800;
/// `if(count>500-16)` (`BP.C:173`): the fade's first count.
const PART2_FADE_START: i16 = 500 - 16;
/// `if(mode==5 && count>500-17) loadpal(pal,768-6)` (`B.C:209`): the faded
/// palette's first upload, `762` bytes (entries 0..253, six bytes short).
const PART2_FADE_UPLOAD_START: i16 = 500 - 17;
/// `if(vram[u]==254-65) vram[u]=1` (`B.C:299`): mode 4's trail index.
const MODE_FOUR_TRAIL: u8 = 254 - 65;

/// `cubepoints` (`BP.C:85-94`): the cube is `-510`..`410`, **not** symmetric —
/// the plan's and the research's "±510" is a transcription error.
pub const CUBE_POINTS: [[i32; 3]; 8] = [
    [-510, -510, -510],
    [410, -510, -510],
    [410, 410, -510],
    [-510, 410, -510],
    [-510, -510, 410],
    [410, -510, 410],
    [410, 410, 410],
    [-510, 410, 410],
];

/// `cubepolys` (`BP.C:95-102`): six quads, colours `0x4001..0x4020`, indices
/// 0..7 of the projected list.
pub const CUBE_POLYS: [PolyDef; 6] = [
    PolyDef {
        colour: 0x4001,
        points: &[3, 2, 1, 0],
    },
    PolyDef {
        colour: 0x4002,
        points: &[4, 5, 6, 7],
    },
    PolyDef {
        colour: 0x4004,
        points: &[0, 1, 5, 4],
    },
    PolyDef {
        colour: 0x4008,
        points: &[1, 2, 6, 5],
    },
    PolyDef {
        colour: 0x4010,
        points: &[2, 3, 7, 6],
    },
    PolyDef {
        colour: 0x4020,
        points: &[3, 0, 4, 7],
    },
];

/// `cubepolyss` (`BP.C:103-110`): the same six quads, every colour `0x4040`,
/// indexing the magnified copy at records 8..15.
#[allow(clippy::identity_op)] // the source's `3+8, 2+8, …, 0+8` form (`BP.C:104-109`)
pub const CUBE_SHADOW_POLYS: [PolyDef; 6] = [
    PolyDef {
        colour: 0x4040,
        points: &[3 + 8, 2 + 8, 1 + 8, 0 + 8],
    },
    PolyDef {
        colour: 0x4040,
        points: &[4 + 8, 5 + 8, 6 + 8, 7 + 8],
    },
    PolyDef {
        colour: 0x4040,
        points: &[0 + 8, 1 + 8, 5 + 8, 4 + 8],
    },
    PolyDef {
        colour: 0x4040,
        points: &[1 + 8, 2 + 8, 6 + 8, 5 + 8],
    },
    PolyDef {
        colour: 0x4040,
        points: &[2 + 8, 3 + 8, 7 + 8, 6 + 8],
    },
    PolyDef {
        colour: 0x4040,
        points: &[3 + 8, 0 + 8, 4 + 8, 7 + 8],
    },
];

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
    /// `gridpoints` (`BP.C:156-164`): the lattice shell, and the `points3`
    /// records `crotprojlist` writes and `do3dots` reads and updates.
    grid: Vec<[i32; 3]>,
    points3: Vec<i16>,
    /// `depthcol` (`BA.ASM:221-222`, `BP.C:128-133`).
    depthcol: [u8; 8192],
    /// `rx/ry/rz/zp` (`BP.C:118`): part 2 sets `zp = 3500`; part 3 continues
    /// the angles (`BP.C:166,248-253`).
    rx: i16,
    ry: i16,
    rz: i16,
    zp: i16,
    /// `static int ct2` in part 3 (`BP.C:260`).
    ct2: i16,
    /// The 128-row opaque compositor (`VNEW.ASM`), seeded from the A2D0 band
    /// at `part3init` — one compositor for the run, as `newdata1`/`vram` are.
    compositor: Compositor,
    /// A48's measured mode-5 boundary: two display frames with no loop
    /// iteration between mode 4's last iteration and part 2's count 0.
    mode_five_hold: u8,
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
            grid: Vec::new(),
            points3: Vec::new(),
            depthcol: [0; 8192],
            rx: 0,
            ry: 0,
            rz: 0,
            zp: 5600,
            ct2: 0,
            compositor: Compositor::new(),
            mode_five_hold: 0,
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

    /// `part2init` (`BP.C:124-167`).
    fn part2init(&mut self) {
        self.depthcol = depthcol_table();
        // `memcpy(pal,defpal,768)`; `pal[1] = (63,63,63)`; `pal[250] =
        // (0,0,40)`; the 64-entry gradient at 64..127 (`BP.C:135-154`). The
        // red channel is `a` itself — the source computes `b=a+d`, clamps it
        // and then stores `a` (`BP.C:145-146`).
        self.pal.copy_from_slice(&self.defpal);
        self.pal[3..6].copy_from_slice(&[63, 63, 63]);
        self.pal[250 * 3..250 * 3 + 3].copy_from_slice(&[0, 0, 40]);
        let mut d: i16 = 64;
        for a in 0..64usize {
            let at = 64 * 3 + a * 3;
            let mut g = a as i16 + 20 + d;
            if g > 63 {
                g = 63;
            }
            let mut b = a as i16 + 40 + d;
            if b > 63 {
                b = 63;
            }
            self.pal[at] = a as u8;
            self.pal[at + 1] = g as u8;
            self.pal[at + 2] = b as u8;
            if d > 0 {
                d -= 8;
            }
        }
        self.loadpal(FULL_UPLOAD_BYTES);
        self.pal2 = self.pal;
        // `gridpoints` and `zp` (`BP.C:156-166`).
        self.grid = grid_points();
        self.points3 = vec![0i16; 2 + 8 * self.grid.len()];
        self.zp = 3500;
    }

    /// One `part2` iteration (`BP.C:169-215`).
    fn part2(&mut self) {
        if self.count >= PART2_END {
            self.nextmode();
        }
        if self.count > PART2_FADE_START {
            // `a=(500-16)-count; fadepal(pal,pal2,a*16)` (`BP.C:173-177`).
            // `a` is negative for every count that reaches here (485..499),
            // so `a*16` is a negative 16-bit `int` and `fadepal` reads it as
            // `>= 256` and clamps `dl` to 255: the fade only dims the palette
            // by 255/256. The capture agrees (reference 1441 onward has
            // blue 251 where it was 255); `docs/fidelity/hard.md` records it.
            let a = PART2_FADE_START.wrapping_sub(self.count);
            fadepal(&mut self.pal, &self.pal2, a.wrapping_mul(16));
        }
        let matrix = matrix_yxz(self.rx, self.ry, self.rz);
        grid_records(
            &mut self.points3,
            &self.grid,
            &matrix,
            [0, 0, i32::from(self.zp)],
        );
        do3dots(
            &mut self.points3,
            &self.depthcol,
            &mut self.screen[BAND_START..BAND_END],
        );
        if self.count > 1 {
            self.rx = self.rx.wrapping_add(9);
            self.ry = self.ry.wrapping_add(10);
            self.rz = self.rz.wrapping_add(11);
            if self.zp > 2000 {
                self.zp -= 10;
            }
        }
    }

    /// `part3init` (`BP.C:220-254`).
    fn part3init(&mut self) {
        self.pal.copy_from_slice(&self.defpal);
        // `pal2[1..127]` is the bit-coded ramp; everything else in `pal2`
        // keeps part 2's palette (`BP.C:224-240`).
        for a in 1..128usize {
            let mut r = 0i16;
            let mut g = 0i16;
            let mut b = 20i16;
            if a & 0x1 != 0 {
                g += 20;
                r += 20;
            }
            if a & 0x2 != 0 {
                g += 22;
                r += 22;
            }
            if a & 0x4 != 0 {
                g += 24;
                r += 24;
            }
            if a & 0x8 != 0 {
                g += 26;
                r += 26;
            }
            if a & 0x10 != 0 {
                g += 28;
                r += 28;
            }
            if a & 0x20 != 0 {
                g += 30;
                r += 30;
            }
            if a & 0x40 != 0 {
                b = 15;
            }
            let at = a * 3;
            self.pal2[at] = r.min(63) as u8;
            self.pal2[at + 1] = g.min(63) as u8;
            self.pal2[at + 2] = b.min(63) as u8;
        }
        // `for(a=2;a<254;a++) pal[a] = (0,0,20)` (`BP.C:241-246`); entry 1
        // stays `defpal[1]` = black. The band's entry 0 is also `(0,0,20)`,
        // so the cube's opposite-face colour pairs XOR into the background's
        // entry on the first frame; the cube appears only as the ramp
        // separates the entries (docs/fidelity/hard.md, part 3).
        for a in 2..254usize {
            let at = a * 3;
            self.pal[at..at + 3].copy_from_slice(&[0, 0, 20]);
        }
        self.loadpal(FULL_UPLOAD_BYTES);
        // `ng_init`'s destination is `gs:vram` = A2D0 (`VNEW.ASM:29-31`):
        // the compositor paints into the band part 2 left and never clears it.
        // The boundary lists start fresh (the source's BSS `newdata1`); the
        // work buffer's rows 128..199 are never painted or displayed.
        let band = self.screen[BAND_START..BAND_END].to_vec();
        self.compositor.pixels[..BAND_BYTES].copy_from_slice(&band);
    }

    /// One `part3` iteration (`BP.C:256-299`).
    fn part3(&mut self) {
        if self.count >= PART3_END {
            self.nextmode();
        }
        if self.muscode(0xff) == 0xff {
            self.nextmode();
        }
        // `ct2+=4; lix=sin1024[ct2&1023]/2+160;
        // liy=sin1024[(ct2+256)&1023]/2+64` (`BP.C:265-267`).
        self.ct2 = self.ct2.wrapping_add(4);
        let lix = SIN1024[(self.ct2 as u16 & 1023) as usize] / 2 + 160;
        let liy = SIN1024[(self.ct2.wrapping_add(256) as u16 & 1023) as usize] / 2 + 64;
        let matrix = matrix_yxz(self.rx, self.ry, self.rz);
        let mut rotated: Vec<[i32; 3]> = Vec::with_capacity(CUBE_POINTS.len());
        rotate(
            &mut rotated,
            &CUBE_POINTS,
            &matrix,
            [0, 0, i32::from(self.zp)],
        );
        let mut verts: Vec<PVert> = Vec::with_capacity(2 * CUBE_POINTS.len());
        project(&mut verts, &rotated);
        let copy = magnified_copy(&verts[..CUBE_POINTS.len()], lix, liy);
        verts.extend(copy);
        // `ceasymode(0)`/Hide for the magnified copy, then
        // `ceasymode(1)`/Norm for the real cube (`BP.C:289-294`).
        let (shadow, cube) = part3_lists(&verts);
        self.compositor.add(&shadow);
        self.compositor.add(&cube);
        self.compositor.paint();
        self.screen[BAND_START..BAND_END].copy_from_slice(&self.compositor.pixels[..BAND_BYTES]);
        self.rx = self.rx.wrapping_add(9);
        self.ry = self.ry.wrapping_add(10);
        self.rz = self.rz.wrapping_add(11);
        if self.count < 20 {
            fadepal(&mut self.pal, &self.pal2, self.count.wrapping_mul(16));
        }
    }

    /// `dis_muscode(wanted)` under the vault's single-start model
    /// (`DIS/DISINT.ASM:275-280`, `APPROXIMATIONS.md` A46): the capture's one
    /// key was consumed by the gate at `B.C:138`, so every later call returns
    /// 0 and part 3's `0xff` cut (`BP.C:263`) cannot fire.
    fn muscode(&self, _wanted: i16) -> i16 {
        0
    }

    /// One main-loop iteration (`B.C:203-324`). Returns true when the part
    /// retires: the `default` arm sets `mode = -1` (`B.C:320-323`).
    fn main_tick(&mut self) -> bool {
        if self.pause {
            self.pause = false;
            return false;
        }
        if self.mode_five_hold > 0 {
            // A48: the artifact's mode-4 loop lost two retraces, so two
            // display frames pass with no loop iteration before part 2's
            // count 0. The port's fixed tick cannot express the overrun
            // inside mode 4; it inserts the same two static displays here so
            // every later frame keeps the artifact's numbering.
            self.mode_five_hold -= 1;
            self.shown = palette_from_mirror(&self.dac);
            return false;
        }
        self.count += 1;
        // `setborder(255)` and `waitb` (`B.C:206-207`): the border variable is
        // 0, so `setborder` returns at `B.C:38`, and one wait is one tick.
        if self.mode == 4 && self.count > 0 {
            // `loadpal(pal,768)` (`B.C:208`): the band built last iteration.
            self.loadpal(FULL_UPLOAD_BYTES);
        }
        if self.mode == 5 && self.count > PART2_FADE_UPLOAD_START {
            // `loadpal(pal,768-6)` (`B.C:209`): 762 bytes, entries 0..253.
            self.loadpal(FULL_UPLOAD_BYTES - 6);
        }
        if self.mode == 6 && self.count > 0 && self.count < 20 {
            // The part-3 fade's upload (`B.C:210`), also 762 bytes.
            self.loadpal(FULL_UPLOAD_BYTES - 6);
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
                    self.mode_five_hold = 2;
                }
            }
            5 => {
                if self.count == 0 {
                    // `for(u=0;u<128*320;u++) if(vram[u]==254-65) vram[u]=1;
                    // else vram[u]=0;` (`B.C:297-301`): the band is cleared
                    // and mode 4's last trail becomes index 1.
                    for pixel in &mut self.screen[BAND_START..BAND_END] {
                        *pixel = if *pixel == MODE_FOUR_TRAIL { 1 } else { 0 };
                    }
                    self.part2init();
                } else {
                    self.part2();
                }
            }
            6 => {
                if self.count == 0 {
                    self.part3init();
                } else {
                    self.part3();
                }
            }
            7 => {
                self.nextmode();
                if self.count > 100 {
                    self.nextmode();
                }
            }
            _ => {
                // `default: mode=-1` (`B.C:320-323`): the loop ends after
                // this iteration, whose frame is the last picture.
                self.mode = -1;
            }
        }
        self.shown = palette_from_mirror(&self.dac);
        self.mode == -1
    }
}

impl Part for Hard {
    fn name(&self) -> &'static str {
        HARD_NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        let raw = read_asset(HARD_NAME, FCRZ_PATH)?;
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
        // the tree (the 60-call loop is `EDGE.C:122`, a different program).
        // The port's first drawn frame matches reference 512 exactly
        // (`docs/fidelity/hard.md` records the scratch burn experiment that
        // decided it).
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
        self.grid.clear();
        self.points3.clear();
        self.depthcol = [0; 8192];
        self.rx = 0;
        self.ry = 0;
        self.rz = 0;
        self.zp = 5600;
        self.ct2 = 0;
        self.compositor = Compositor::new();
        self.mode_five_hold = 0;
        // The mode set leaves this DAC in place until the pre-roll's first
        // `loadpal(defpal,768)`.
        self.shown = crate::shell::hardware_default_palette();
        *ctx.view = mode_view(
            SCREEN_WIDTH as u16,
            SCREEN_HEIGHT as u16,
            Some(SCREEN_WIDTH as u16),
            Some(0),
        );
        ctx.set_palette(self.shown);
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // The source polls `dis_exit` at exactly two places: the 70-wait loop
        // (`B.C:162`) and the main loop's condition (`B.C:203`). The wipe and
        // the fill (`B.C:143-161`) do not poll, so the port does not end
        // there either; a flag set during them is seen by the next wait.
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
                if ctx.exit_requested {
                    // The wait loop's condition breaks; the pre-roll then
                    // continues through the letterbox (`B.C:164-182`) and the
                    // main loop exits at its own check.
                    self.phase = Phase::Letterbox;
                } else {
                    self.wait_tick();
                }
                false
            }
            Phase::Letterbox => {
                self.letterbox_tick();
                false
            }
            Phase::Main => {
                // `while(!dis_exit() && mode!=-1)`: the first tick with the
                // flag set runs no iteration and retires the part.
                if ctx.exit_requested {
                    true
                } else {
                    self.main_tick()
                }
            }
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

// ----------------------------------------------------------- part 2/3 data --

/// `crotprojlist`'s record layout (`VEC.ASM:250-344`, `BP.C:183`): a 16-bit
/// count at word 0 and 16-byte records — x at word 0, y at word 1 and the
/// 32-bit z at words 4/5 — for [`do3dots`].
///
/// The record's other words are **not** written by the projection: word 3 is
/// the last screen offset and word 6 the shadow offset, both owned by
/// `do3dots`. They live in the caller's `points3`, so they persist across
/// frames exactly as the source's BSS array does.
pub fn grid_records(
    points3: &mut [i16],
    grid: &[[i32; 3]],
    matrix: &Matrix,
    add: [i32; 3],
) -> usize {
    let mut verts: Vec<PVert> = Vec::with_capacity(grid.len());
    rotate_project(&mut verts, grid, matrix, add);
    points3[0] = verts.len() as i16;
    for (index, vert) in verts.iter().enumerate() {
        let at = 2 + index * 8;
        points3[at] = vert.x;
        points3[at + 1] = vert.y;
        points3[at + 4] = vert.z as i16;
        points3[at + 5] = (vert.z >> 16) as i16;
    }
    verts.len()
}

/// The magnified copy's projection (`BP.C:275-288`): each projected cube
/// point is pulled toward `(lix, liy)` by `z' = 3000` over `z + 2000`.
///
/// The source's `z` is a `long` assigned from `points3[...+4]`, an `int` — so
/// it is the **low word** of the projected z, sign-extended, plus 2000, not
/// the 32-bit z (`BP.C:280-281`). `x-lix` is likewise an `int` and wraps in
/// 16 bits before the cast to `long`; the quotient truncates toward zero and
/// the `+ lix` result is stored back through an `int`.
pub fn magnified_copy(verts: &[PVert], lix: i16, liy: i16) -> Vec<PVert> {
    verts
        .iter()
        .map(|vert| {
            let dx = i32::from(vert.x.wrapping_sub(lix));
            let dy = i32::from(vert.y.wrapping_sub(liy));
            let z = i32::from(vert.z as i16) + 2000;
            // The source would fault on a zero divisor; part 3's z' is
            // 3118..4882, so this guard models an unreachable branch.
            let x = (dx * 5000)
                .checked_div(z)
                .unwrap_or(0)
                .wrapping_add(i32::from(lix)) as i16;
            let y = (dy * 5000)
                .checked_div(z)
                .unwrap_or(0)
                .wrapping_add(i32::from(liy)) as i16;
            PVert {
                x,
                y,
                flags: 0,
                z: 3000,
                // The second block's raw fields are BSS zero: the source's
                // copy loop writes only x, y and the z dword (`BP.C:285-287`),
                // and the clipper's interpolation is unreachable for it
                // (every z' is 3000, below `clipz`).
                raw_x: 0,
                raw_y: 0,
            }
        })
        .collect()
}

/// The two clipper passes (`BP.C:289-294`): `ceasymode(0)`/`demo_hide` for
/// the magnified copy (`CUBE_SHADOW_POLYS`, records 8..15) and
/// `ceasymode(1)`/`demo_norm` for the real cube (`CUBE_POLYS`, records 0..7).
pub fn part3_lists(verts: &[PVert]) -> (Vec<Poly>, Vec<Poly>) {
    (
        clip_easy_list(&CUBE_SHADOW_POLYS, verts, FaceMode::Hide),
        clip_easy_list(&CUBE_POLYS, verts, FaceMode::Norm),
    )
}

/// `fadepal` (`BA.ASM:174-215`): every source channel times `dl` in the high
/// byte of an 8-bit multiply, plus `20*(255-dl)/256` added to blue.
///
/// `dl` is a 16-bit `int` and the range test is unsigned (`cmp dx,256; jb`),
/// so a negative or oversized value clamps to 255.
pub fn fadepal(dst: &mut [u8; 768], src: &[u8; 768], dl: i16) {
    let dl: u8 = if (dl as u16) < 256 { dl as u8 } else { 255 };
    let blue_bias = ((20 * u32::from(255 - dl)) >> 8) as u8;
    for (out, source) in dst.chunks_exact_mut(3).zip(src.chunks_exact(3)) {
        out[0] = ((u32::from(source[0]) * u32::from(dl)) >> 8) as u8;
        out[1] = ((u32::from(source[1]) * u32::from(dl)) >> 8) as u8;
        out[2] = (((u32::from(source[2]) * u32::from(dl)) >> 8) as u8).wrapping_add(blue_bias);
    }
}

// ------------------------------------------------------------- palettes --

/// A DAC mirror as an engine [`Palette`]. The bytes are already 6-bit.
fn palette_from_mirror(bytes: &[u8; 768]) -> Palette {
    Palette::from_six_bit(bytes)
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

    use crate::parts::vault::hard::poly::shoelace;

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
        for part_frame in 0..700u32 {
            let status = tick(&mut engine, &mut part, &mut frame);
            if (part.mode, part.count) != previous {
                seen.push((part_frame, part.mode, part.count));
                previous = (part.mode, part.count);
            }
            assert_eq!(
                status,
                PartStatus::Continuing,
                "the first half does not retire; modes 5..6 run to port 1902"
            );
        }
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
        // Mode 4's `count>100` branch (`B.C:288-292`) kills the dots and
        // advances at count 101; the post-state is mode 5, count -1, and the
        // two measured boundary displays follow (A48).
        let advance = seen
            .iter()
            .find(|(_, mode, count)| *mode == 5 && *count == -1);
        assert_eq!(
            advance.map(|(frame, _, _)| *frame),
            Some(mode_four_start + 101),
            "mode 4 runs counts 0..101 and advances on the last"
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

    #[test]
    fn the_exit_flag_is_polled_where_the_source_polls_it() {
        let (mut engine, mut part, mut frame) = start();
        // `B.C:143-161` does not poll: a flag set before the wipe runs the
        // wipe and the fill to the waits.
        engine.request_exit();
        for _ in 0..20 {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
        }
        assert!(matches!(part.phase, Phase::Fill { .. }));
        for _ in 0..28 {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
        }
        assert!(matches!(part.phase, Phase::Wait { .. }));

        // The wait loop's condition (`B.C:162`) breaks without consuming a
        // wait; the letterbox still runs, then the main loop's first check
        // retires the part with no iteration (`B.C:203`).
        assert_eq!(
            tick(&mut engine, &mut part, &mut frame),
            PartStatus::Continuing
        );
        assert_eq!(part.phase, Phase::Letterbox);
        assert_eq!(
            tick(&mut engine, &mut part, &mut frame),
            PartStatus::Continuing
        );
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(
            tick(&mut engine, &mut part, &mut frame),
            PartStatus::Finished
        );
        assert_eq!(
            (part.mode, part.count),
            (0, -1),
            "the main loop ran no iteration"
        );
    }

    /// The part-2 projection is `crotprojlist` (`VEC.ASM:250-344`): the same
    /// rotation as `rotlist`, then a 16-bit `imul word ptr`/`idiv bx`
    /// projection that reads the low words only and writes no raw fields.
    #[test]
    fn the_part2_projection_is_the_16_bit_crotprojlist() {
        let identity = Matrix([32767, 0, 0, 0, 32767, 0, 0, 0, 32767]);
        let mut points3 = vec![0i16; 2 + 8];
        let count = grid_records(&mut points3, &[[200, 200, 300]], &identity, [0, 0, 65536]);
        assert_eq!(count, 1);
        assert_eq!(points3[0], 1);
        // HARD's axis stores rotate (200,200,300) to (299,199,199); a z
        // translation of 65536 lifts z to 65735, whose low word is 199:
        // x = 299*240/199 + 160 = 520, y = 199*200/199 + 64 = 264.
        assert_eq!((points3[2], points3[3]), (520, 264));
        // The record carries the dword z at words 4/5; `do3dots` loads word
        // 4 (`BA.ASM:232`), which is 199 here.
        assert_eq!((points3[6], points3[7]), (199, 1));
        // The 32-bit `projlist` on the same rotated point divides by 65735
        // and gives (161, 64); the part must not take that path.
        let mut wide = Vec::new();
        project(&mut wide, &[[299, 199, 65_735]]);
        assert_eq!((wide[0].x, wide[0].y), (161, 64));
        assert_ne!((points3[2], points3[3]), (wide[0].x, wide[0].y));
    }

    /// The first part-2 frame's drawn list is the shipped `GRID1` table: at
    /// `zp = 3500`, `rx = ry = rz = 0`, only the lattice face `x = -500`
    /// passes `do3dots`' signed `z <= 3000` test, and its projection is
    /// exactly the 100 pairs mode 4's trails end on.
    #[test]
    fn the_first_grid_frame_is_the_shipped_grid1_list() {
        let grid = grid_points();
        let matrix = matrix_yxz(0, 0, 0);
        let mut points3 = vec![0i16; 2 + 8 * grid.len()];
        grid_records(&mut points3, &grid, &matrix, [0, 0, 3500]);
        let mut drawn = Vec::new();
        for index in 0..grid.len() {
            let at = 2 + index * 8;
            if points3[at + 4] <= 3000 {
                drawn.push((points3[at], points3[at + 1]));
            }
        }
        assert_eq!(drawn.len(), 100, "the x = -500 face");
        let table: Vec<(i16, i16)> = GRID1
            .chunks_exact(2)
            .map(|pair| (pair[0], pair[1]))
            .collect();
        assert_eq!(drawn, table);
        // Every drawn point uses `depthcol[3000] = 65`, and `part2init`'s
        // entry 65 is `(1,63,63)` — the capture's `(4,255,255)` at reference
        // 956 (`docs/fidelity/hard.md`).
        assert_eq!(depthcol_table()[3000], 65);
        let mut part = Hard::new();
        part.part2init();
        assert_eq!(&part.pal[65 * 3..65 * 3 + 3], &[1, 63, 63]);
        assert_eq!(&part.pal[3..6], &[63, 63, 63]);
        assert_eq!(&part.pal[250 * 3..250 * 3 + 3], &[0, 0, 40]);
    }

    /// A51's measured domain: part 2's 501 states (`BP.C:169-215`) walked
    /// through the real recurrence — `part2init`'s count 0, then `part2`'s
    /// counts 1..=500, the angles advancing after every count above 1 and
    /// `zp` falling ten a step to its 2000 floor. The record's full z′ range,
    /// the drawn low-word bound and the first frame's `depthcol[3000]` face
    /// all come from this walk, not from the note.
    #[test]
    fn the_part2_grid_z_range_and_drawn_words_are_the_measured_domain() {
        let mut part = Hard::new();
        part.part2init();
        assert_eq!(part.grid.len(), 488);
        assert_eq!((part.rx, part.ry, part.rz, part.zp), (0, 0, 0, 3500));

        let mut full = (i32::MAX, i32::MIN);
        let mut drawn = (i16::MAX, i16::MIN);
        let mut drawn_count = 0usize;
        let mut first_drawn: Vec<i16> = Vec::new();
        let mut projected_states = 0;

        // Count 0 is `part2init` alone; counts 1..=500 each project one state
        // (`BP.C:178-213`).
        for count in 1..=500i16 {
            part.count = count;
            part.part2();
            projected_states += 1;
            for index in 0..part.grid.len() {
                let at = 2 + index * 8;
                // The record's dword z at words 4/5 (`VEC.ASM:326-335`); word
                // 5 must stay 0 for the low word to be the whole z′.
                assert_eq!(part.points3[at + 5], 0, "count {count}: z high word");
                let z = u32::from(part.points3[at + 4] as u16) as i32;
                full = (full.0.min(z), full.1.max(z));
                let low = part.points3[at + 4];
                // `do3dots`' signed `cmp bp,3000; jg` (`BA.ASM:233-235`):
                // every point that reaches `depthcol` has a positive low word
                // no larger than 3000.
                if low <= 3000 {
                    assert!(low >= 1, "count {count}: drawn low word {low}");
                    drawn = (drawn.0.min(low), drawn.1.max(low));
                    drawn_count += 1;
                    if count == 1 {
                        first_drawn.push(low);
                    }
                }
            }
        }
        assert_eq!(projected_states, 500, "counts 1..=500");
        // Count 0 plus the 500 projections are part 2's 501 states; counts 1
        // and 2 project the same state because the advance follows the draw.
        assert_eq!(first_drawn.len(), 100, "the first frame's x = -500 face");
        assert!(first_drawn.iter().all(|&low| low == 3000));
        // The range is the walk's, not the loose `zp ± 500*√3` envelope A51
        // once carried: `zp` is 3500 only while the angles are still zero (so
        // z′ is the lattice x plus 3500, at most 3900), and by the time the
        // rotation can align a corner with z the translation has fallen. The
        // measured maximum is 3964 and the minimum 1139.
        assert_eq!((full.0, full.1), (1139, 3964), "full z′ range");
        assert_eq!((drawn.0, drawn.1), (1139, 3000), "drawn low-word range");
        assert_eq!(drawn_count, 218_961, "z-passing points over 500 projections");
    }

    /// `cubepoints`/`cubepolys`/`cubepolyss` and the magnified copy
    /// (`BP.C:85-110,275-288`).
    #[test]
    fn the_cube_geometry_and_colours_are_the_sources() {
        // The cube is -510..410, not ±510: the plan's and the research's
        // "±510" is a transcription error.
        assert_eq!(CUBE_POINTS[0], [-510, -510, -510]);
        assert_eq!(CUBE_POINTS[4], [-510, -510, 410]);
        assert_eq!(CUBE_POINTS[7], [-510, 410, 410]);
        for point in CUBE_POINTS {
            for coordinate in point {
                assert!(coordinate == -510 || coordinate == 410, "{point:?}");
            }
        }
        // The six colour words in table order (`BP.C:95-102`).
        let colours: Vec<u16> = CUBE_POLYS.iter().map(|poly| poly.colour).collect();
        assert_eq!(colours, [0x4001, 0x4002, 0x4004, 0x4008, 0x4010, 0x4020]);
        // The +8 copy: the same indices in the same order, all 0x4040
        // (`BP.C:103-110`).
        for (real, shadow) in CUBE_POLYS.iter().zip(&CUBE_SHADOW_POLYS) {
            assert_eq!(shadow.colour, 0x4040);
            assert_eq!(shadow.points.len(), real.points.len());
            for (far, near) in shadow.points.iter().zip(real.points) {
                assert_eq!(*far, near + 8);
            }
        }

        // The magnified copy's projection: the denominator is the **low
        // word** of z plus 2000, z' is 3000, and the quotient truncates
        // toward zero.
        let verts: Vec<PVert> = (0..8)
            .map(|_| PVert {
                x: 250,
                y: 70,
                flags: 0,
                z: 5000 + 65536,
                raw_x: 0,
                raw_y: 0,
            })
            .collect();
        let copy = magnified_copy(&verts, 190, 80);
        assert_eq!(copy.len(), 8);
        // (250-190)*5000/(5000+2000) = 42, +190 = 232;
        // (70-80)*5000/7000 = -7, +80 = 73.
        assert_eq!((copy[0].x, copy[0].y), (232, 73));
        assert_eq!(copy[0].z, 3000);
        // A 32-bit z would divide by 72536 and give 194 instead.
        let wide = (250i32 - 190) * 5000 / (5000 + 65536 + 2000) + 190;
        assert_eq!(wide, 194);
        assert_ne!(i32::from(copy[0].x), wide);
    }

    /// `ceasymode(0)` selects `demo_hide` and `ceasymode(1)` `demo_norm`
    /// (`VEC.ASM:904-916`): part 3 passes the magnified copy through Hide and
    /// the real cube through Norm (`BP.C:289-294`).
    #[test]
    fn the_part3_mode_selects_hide_and_norm() {
        let matrix = matrix_yxz(0, 0, 0);
        let mut rotated = Vec::new();
        rotate(&mut rotated, &CUBE_POINTS, &matrix, [0, 0, 2000]);
        let mut verts = Vec::new();
        project(&mut verts, &rotated);
        let copy = magnified_copy(&verts[..8], 190, 80);
        verts.extend(copy);

        let (shadow, cube) = part3_lists(&verts);
        assert_eq!(
            shadow,
            clip_easy_list(&CUBE_SHADOW_POLYS, &verts, FaceMode::Hide)
        );
        assert_eq!(cube, clip_easy_list(&CUBE_POLYS, &verts, FaceMode::Norm));
        assert_eq!(shadow.len(), 6);
        assert_eq!(cube.len(), 6);
        // Norm keeps every face's colour.
        assert!(cube.iter().all(|poly| poly.colour != 0));
        // Hide zeroes exactly the faces whose shoelace is negative, on the
        // same block and on the magnified copy.
        let norm = clip_easy_list(&CUBE_POLYS, &verts, FaceMode::Norm);
        let hide = clip_easy_list(&CUBE_POLYS, &verts, FaceMode::Hide);
        for (visible, hidden) in norm.iter().zip(&hide) {
            assert_eq!(visible.points, hidden.points);
            assert_eq!(
                hidden.colour == 0,
                shoelace(&visible.points) < 0,
                "{:?}",
                visible.points
            );
        }
        let shadow_zeroed = shadow.iter().filter(|poly| poly.colour == 0).count();
        let cube_zeroed = hide.iter().filter(|poly| poly.colour == 0).count();
        assert!(
            (1..6).contains(&cube_zeroed),
            "the identity view shows both signs: {cube_zeroed}"
        );
        assert!(
            (0..6).contains(&shadow_zeroed),
            "the magnified copy clips its own faces: {shadow_zeroed}"
        );
    }

    /// The band is the `A2D0` work buffer the source's compositor paints
    /// into; `ng_init` never touches a pixel (`VNEW.ASM:27-56`), so part 3
    /// must seed its compositor from part 2's band and leave untouched spans
    /// alone.
    #[test]
    fn the_band_is_not_cleared_between_frames() {
        let (mut engine, mut part, mut frame) = start();
        while !(part.mode == 6 && part.count == 0) {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
        }
        // Plant a sentinel before the init seeds the compositor, so the seed
        // is visible; pixel 0 is far outside the cube (the centre is 160,64
        // and the cube spans about ±80).
        part.screen[BAND_START] = 99;
        part.part3init();
        assert_eq!(part.compositor.pixels[0], 99, "seeded from the A2D0 band");
        let seeded = part.compositor.pixels;
        part.count = 1;
        part.part3();
        assert_ne!(part.compositor.pixels, seeded, "the cube painted");
        assert_eq!(part.screen[BAND_START], 99, "an untouched span keeps it");
        part.count = 2;
        part.part3();
        assert_eq!(part.screen[BAND_START], 99, "and the next frame too");
    }

    #[test]
    fn fadepal_is_the_high_byte_multiply() {
        let mut src = [0u8; 768];
        src[3..6].copy_from_slice(&[63, 63, 63]);
        src[6..9].copy_from_slice(&[1, 63, 63]);
        let mut dst = [0u8; 768];
        // dl = 0: every channel zero, blue takes the whole bias 19.
        fadepal(&mut dst, &src, 0);
        assert_eq!(&dst[3..6], &[0, 0, 19]);
        assert_eq!(&dst[6..9], &[0, 0, 19]);
        // dl = 16: 63*16 >> 8 = 3, bias 20*239 >> 8 = 18.
        fadepal(&mut dst, &src, 16);
        assert_eq!(&dst[3..6], &[3, 3, 21]);
        assert_eq!(&dst[6..9], &[0, 3, 21]);
        // dl = 128: 63*128 >> 8 = 31, bias 20*127 >> 8 = 9.
        fadepal(&mut dst, &src, 128);
        assert_eq!(&dst[3..6], &[31, 31, 40]);
        // dl = 255: no bias, the channel loses its low bit.
        fadepal(&mut dst, &src, 255);
        assert_eq!(&dst[3..6], &[62, 62, 62]);
        // 256 and a negative int both clamp to 255 (`cmp dx,256; jb` is an
        // unsigned compare) — the mode-5 fade's `a*16` goes negative.
        for dl in [256i16, -16, -240] {
            fadepal(&mut dst, &src, dl);
            assert_eq!(&dst[3..6], &[62, 62, 62], "dl {dl}");
        }
    }

    #[test]
    fn the_full_mode_sequence_is_the_sources() {
        let (mut engine, mut part, mut frame) = start();
        // The plan's sum of the mode limits.
        assert_eq!(34 + 4 * 73 + 4 + 42 + 102 + 500 + 800, 1774);
        let mut transitions: Vec<(u32, i16, i16)> = Vec::new();
        let mut previous = (part.mode, part.count);
        let mut finished_at = None;
        for tick_index in 0..2000u32 {
            let status = tick(&mut engine, &mut part, &mut frame);
            if (part.mode, part.count) != previous {
                transitions.push((tick_index, part.mode, part.count));
                previous = (part.mode, part.count);
            }
            if status == PartStatus::Finished {
                finished_at = Some(tick_index);
                break;
            }
        }
        assert_eq!(finished_at, Some(1902), "the `default` arm's tick");
        assert_eq!(part.mode, -1);
        let has = |tick_index: u32, mode: i16, count: i16| {
            transitions.contains(&(tick_index, mode, count))
        };
        // 119 pre-roll ticks; modes 0..4 (478 ticks); the two measured A48
        // boundary displays 953-954; part 2's 501 iterations (counts 0..500,
        // reference 955..1455); part 3's 801 (counts 0..800, reference
        // 1456..2256); mode 7 and `default` (references 2257-2258).
        assert!(has(596, 5, -1), "mode 4's advance (reference 952)");
        assert!(has(599, 5, 0), "part2init (reference 955)");
        assert!(has(600, 5, 1), "the first grid frame (reference 956)");
        assert!(has(1099, 6, -1), "part 2's count 500 advances");
        assert!(has(1100, 6, 0), "part3init (reference 1456)");
        assert!(has(1101, 6, 1), "the first cube frame (reference 1457)");
        assert!(has(1900, 7, -1), "part 3's count 800 advances");
        assert!(has(1901, 8, -1), "mode 7's one iteration");
        assert!(has(1902, -1, 0), "the `default` arm");
        assert_eq!(119 + 478 + 2 + 501 + 801 + 2, 1903);
    }

    #[test]
    fn the_0xff_cut_is_unreachable_with_one_start() {
        let (mut engine, mut part, mut frame) = start();
        // The vault run delivers no later key: the standalone DIS's one echo
        // was consumed by the gate (`B.C:138`), so every later `muscode`
        // returns 0 (`DIS/DISINT.ASM:275-280`; A46).
        assert_eq!(part.muscode(0xff), 0);
        assert_ne!(part.muscode(0xff), 0xff);
        while !(part.mode == 6 && part.count == 0) {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
        }
        // Part 3 reaches its own limit: counts 1..799 leave it in mode 6 and
        // count 800 advances it (`BP.C:262`).
        for count in 1..800 {
            assert_eq!(
                tick(&mut engine, &mut part, &mut frame),
                PartStatus::Continuing
            );
            assert_eq!((part.mode, part.count), (6, count), "count {count}");
        }
        tick(&mut engine, &mut part, &mut frame);
        assert_eq!(part.mode, 7, "count 800 is the source's own end");
    }
}
