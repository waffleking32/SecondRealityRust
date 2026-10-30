//! GLENZ (`GLENZ/MAIN.C` over the part's own `VEC.ASM`, `MATH.ASM`,
//! `MATHSIN.INC` and `NEW.ASM`).
//!
//! GLENZ carries its own vector and span library rather than using Phase 6's
//! `VISU`: the two are different engines, with different fixed-point units,
//! different projection variables and a different compositor. The module tree
//! grows a file per layer of that library:
//!
//! - `math` — `cmatrix_yxz`/`calcmatrix`, `crotlist`, `cprojlist` and
//!   `ccliplist` (`MATH.ASM`, `VEC.ASM`), with the asm's 16/32/64-bit widths.
//! - `tables` — the source's `sintable16`/`costable16` (`MATHSIN.INC`) as
//!   [`tables::SIN`] and [`tables::COS`]. `ZOOM.ASM`'s `_sin1024` is the
//!   shared [`SIN1024`].
//! - `poly` — `ceasypolylist`, the hidden-face cross product and the
//!   `rolcol`/`rolused` colour allocator (`VEC.ASM:260-553`) with the shipped
//!   polygon tables (`MAIN.C:176-240`).
//! - `span` — the transparent span compositor (`NEW.ASM:26-418`).
//! - `intro` — `zoomer2` (`ZOOMER.C:18-81`) and the FC floor bounce
//!   (`MAIN.C:320-350`).
//!
//! # The buffer and the beam
//!
//! The port keeps the original's two 320-pixel-linear buffers: `vram` is
//! 320x400 (the inherited Mode X screen the zoomer closes over, then the
//! 320x200 mode-13h screen), and `bgpic` is the 64,000-byte picture the span
//! compositor paints over (`MAIN.C:12`, `:18`). The original's plane masks and
//! the shipped 92-byte CRTC row are memory-layout facts, not image facts
//! (`docs/fidelity/reference-indices.md` §7.1; D36): every tick the part
//! copies the buffer's rows into the engine surface's first 320 columns and
//! lets the view read them.
//!
//! # The tick model (D37)
//!
//! One logic tick is one `dis_waitb` (`MAIN.C:413`; the wrapper is
//! `DIS/DISC.ASM:42-47`). The measured anchors
//! (`second2b`, `reference-indices.md` §7.6) are: the music wait exits at
//! **1111**, and `zoomer2`'s first body is that same tick because the source's
//! post-wait actions run in the call that saw the wait end. The 47 zoomer
//! draws then occupy 1111..1157; the 48th call breaks, runs the epilogue and
//! the mode set, and draws the bounce's first band, all in 1158; the bounce's
//! 177 draws occupy 1158..1334. `getmframe() < 300` first fails at **1515**
//! (404 ticks after the reset), the palette build is the next tick, the
//! snapshot the one after, `getmframe() < 333` first fails at **1560**, the
//! part-start wait is 1561, and the main loop's first wait is **1562**. The
//! body after that wait is drawn in 1563 — the source's `repeat=dis_waitb()`
//! comes *before* the frame's work — which is what puts main-loop frame 44's
//! first object pixels on the measured **1606** (`L_r = 1606 - 44 = 1562`).
//!
//! # The palette
//!
//! The source's copper callback uploads `pal`'s entries 0..15 at every
//! retrace (`MAIN.C:264-270`, `:408`). The port does not register a copper
//! callback: its per-tick entry-0..15 upload plus `poly::build`'s group writes
//! are the same DAC state at compose time (D37; `docs/fidelity/glenz.md`).
//! Entries 16..255 are only ever written by the colour allocator's 16-entry
//! groups, and the per-tick upload must never touch them.
//!
//! # The exit
//!
//! `while(frame<7000 && !dis_exit())` with `a=dis_musplus(); if(a<0 && a>-16)
//! break;` (`MAIN.C:409-411`): the part ends at the top of a main-loop tick on
//! ESC, on the music cut, or after frame 7000, and leaves the screen as it
//! stands. It has no teardown, so `drains_after_exit` is false.

pub mod intro;
pub mod math;
pub mod poly;
pub mod span;
pub mod tables;


use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use intro::{Bounce, Zoomer, SCREEN_PIXELS, VRAM_PIXELS};
use math::{clip_y, matrix_yxz, project, rotate, Matrix, PVert, INIT_320X200};
use poly::{build, ColourState, DemoMode, EPOLYS, EPOLYSB};
use span::Compositor;
use crate::parts::sin1024::SIN1024;
use crate::parts::{asset, load_image, mode_view};

/// The part's name: what `--part` accepts and the registry lists.
const NAME: &str = "glenz";

/// The FC-logo picture (`Uh1`, 320x200, palette 6-bit; `MAIN.C:14`).
const PICTURE: &str = "assets/GLENZ/FC.UH";

const VIEW_WIDTH: u16 = 320;
/// BEGLOGO's inherited Mode X screen (`MAIN.C:288-304`).
const VIEW_HEIGHT_400: u16 = 400;
/// `int 10h` mode 13h (`MAIN.C:299-303`).
const VIEW_HEIGHT_200: u16 = 200;

/// The 320x200 frame after the mode set.
const MAIN_ROWS: usize = 200;

/// `zpos` (`MAIN.C:277`): fixed for the whole part.
const ZPOS: i16 = 7500;
/// `points`' cube half-edge (`MAIN.C:57-58`): 100*ZZZ and 170*ZZZ.
const ZZZ: i32 = 50;
/// `pointsb`'s cube half-edge: 60*QQQ and 105*QQQ (`MAIN.C:57-58`).
const QQQ: i32 = 99;

/// `points` (`MAIN.C:60-74`): the first object's 14 vertices, the three axis
/// spikes at ±170*ZZZ and the eight cube corners at ±100*ZZZ.
pub(crate) const POINTS: [[i32; 3]; 14] = [
    [-100 * ZZZ, -100 * ZZZ, -100 * ZZZ],
    [100 * ZZZ, -100 * ZZZ, -100 * ZZZ],
    [100 * ZZZ, 100 * ZZZ, -100 * ZZZ],
    [-100 * ZZZ, 100 * ZZZ, -100 * ZZZ],
    [-100 * ZZZ, -100 * ZZZ, 100 * ZZZ],
    [100 * ZZZ, -100 * ZZZ, 100 * ZZZ],
    [100 * ZZZ, 100 * ZZZ, 100 * ZZZ],
    [-100 * ZZZ, 100 * ZZZ, 100 * ZZZ],
    [0, 0, -170 * ZZZ],
    [0, 0, 170 * ZZZ],
    [170 * ZZZ, 0, 0],
    [-170 * ZZZ, 0, 0],
    [0, 170 * ZZZ, 0],
    [0, -170 * ZZZ, 0],
];

/// `pointsb` (`MAIN.C:135-149`): the second object, a 60*QQQ cube with
/// 105*QQQ spikes.
pub(crate) const POINTSB: [[i32; 3]; 14] = [
    [-60 * QQQ, -60 * QQQ, -60 * QQQ],
    [60 * QQQ, -60 * QQQ, -60 * QQQ],
    [60 * QQQ, 60 * QQQ, -60 * QQQ],
    [-60 * QQQ, 60 * QQQ, -60 * QQQ],
    [-60 * QQQ, -60 * QQQ, 60 * QQQ],
    [60 * QQQ, -60 * QQQ, 60 * QQQ],
    [60 * QQQ, 60 * QQQ, 60 * QQQ],
    [-60 * QQQ, 60 * QQQ, 60 * QQQ],
    [0, 0, -105 * QQQ],
    [0, 0, 105 * QQQ],
    [105 * QQQ, 0, 0],
    [-105 * QQQ, 0, 0],
    [0, 105 * QQQ, 0],
    [0, -105 * QQQ, 0],
];

/// The low word of two signed 16-bit words' product: the source's `int`
/// arithmetic truncates every product at 16 bits (`imul`'s `DX:AX`, with only
/// `AX` kept when the C result is an `int` — `disassembly` in
/// `docs/fidelity/glenz.md` §"The physics").
fn mul16(a: i16, b: i16) -> i16 {
    a.wrapping_mul(b)
}

/// `int / int`: truncating toward zero, as `cwd; idiv` does.
fn div16(a: i16, b: i16) -> i16 {
    a / b
}

/// `SIN1024[(a*n) & 1023]` with the 16-bit product's low word (`MAIN.C:456`).
fn sin1024(a: i16, n: i16) -> i16 {
    let index = (a.wrapping_mul(n) as u16 & 1023) as usize;
    SIN1024[index]
}

/// The main loop's per-frame state and physics (`MAIN.C:409-590`).
///
/// Every variable the source declares `int` is 16-bit there, and several
/// expressions wrap: the bounce's `-yposa*boingm` passes 16 bits around frame
/// 667, and `oyp` accumulates `-b*b/2` past 32767 from frame 1801. Those
/// wraps are part of the shipped animation and are reproduced here, which is
/// why the fields are `i16` and the products go through [`mul16`].
pub(crate) struct MainState {
    /// The source's `frame` (`MAIN.C:283`): 0 until the first main body.
    pub frame: i16,
    rx: i16,
    ry: i16,
    rz: i16,
    ypos: i16,
    yposa: i16,
    jello: i16,
    jelloa: i16,
    boingm: i16,
    boingd: i16,
    xscale: i16,
    yscale: i16,
    zscale: i16,
    bscale: i16,
    oxp: i16,
    oyp: i16,
    ozp: i16,
    oxb: i16,
    oyb: i16,
    ozb: i16,
    /// `pal` (`MAIN.C:245`): the 16 entries the copper and the loop upload.
    pub pal: [[u8; 3]; 16],
    /// `backpal` (`MAIN.C:244`): the picture's entries 0..15, rebuilt at
    /// frames 786..789 and not again.
    pub backpal: [[u8; 3]; 16],
    /// `_lightshift` (`MAIN.C:394`, `:500`): 9, or 10 once the second
    /// object's scale overtakes the first's. Mirrored into [`ColourState`].
    pub lightshift: i16,
}

impl MainState {
    /// The initialisers of `MAIN.C:273-287`, with `backpal` already read from
    /// `FC.UH`.
    pub(crate) fn new(backpal: [[u8; 3]; 16]) -> MainState {
        MainState {
            frame: 0,
            rx: 0,
            ry: 0,
            rz: 0,
            // `zpos=7500; ypos=-9000; yposa=0;`
            ypos: -9000,
            yposa: 0,
            jello: 0,
            jelloa: 0,
            boingm: 6,
            boingd: 7,
            xscale: 120,
            yscale: 120,
            zscale: 120,
            bscale: 0,
            oxp: 0,
            oyp: 0,
            ozp: 0,
            oxb: 0,
            oyb: 0,
            ozb: 0,
            pal: backpal,
            backpal,
            lightshift: 9,
        }
    }

    /// `lightshift=9; rx=ry=rz=0; ypos=-9000; yposa=0;` (`MAIN.C:394-396`).
    fn reset_positions(&mut self) {
        self.lightshift = 9;
        self.rx = 0;
        self.ry = 0;
        self.rz = 0;
        self.ypos = -9000;
        self.yposa = 0;
    }

    /// The first object's `if(xscale>4)` (`MAIN.C:595`).
    pub(crate) fn first_object_on(&self) -> bool {
        self.xscale > 4
    }

    /// The second object's `if(frame>800 && bscale>4)` (`MAIN.C:618`).
    pub(crate) fn second_object_on(&self) -> bool {
        self.frame > 800 && self.bscale > 4
    }

    /// One frame of `MAIN.C:417-591`'s inner body: `frame++`, the rotations,
    /// the physics, the palette phases and the frame-765..789 band clear.
    ///
    /// The palette upload that precedes the work is the part's, not this
    /// function's: the source uploads `pal` and *then* computes the new one
    /// (`:414-590`), so the frame displays the previous tick's palette.
    pub(crate) fn step(&mut self, bgpic: &mut [u8], vram: &mut [u8]) {
        self.frame = self.frame.wrapping_add(1);
        let frame = self.frame;
        // `rx+=32; ry+=7; rx%=3*3600; ry%=3*3600; rz%=3*3600;`
        // (`MAIN.C:419-421`).
        self.rx = (self.rx.wrapping_add(32)) % 10800;
        self.ry = (self.ry.wrapping_add(7)) % 10800;
        self.rz %= 10800;

        if frame > 900 {
            // `a=frame-900; b=frame-900; if(b>50) b=50;` (`:454-455`).
            let a = frame.wrapping_sub(900);
            let b = if a > 50 { 50 } else { a };
            // `oxp=sin1024[(a*3)&1023]*b/10;` (`:456-458`).
            self.oxp = div16(mul16(sin1024(a, 3), b), 10);
            self.oyp = div16(mul16(sin1024(a, 5), b), 10);
            // `ozp=(sin1024[(a*4)&1023]/2+128)*b/16;`
            let half = div16(sin1024(a, 4), 2);
            self.ozp = div16(mul16(half.wrapping_add(128), b), 16);
            if frame > 1800 {
                // `a=frame-1800+64; if(a>1024) a=1024;` and the three
                // `(long)` products (`:459-466`).
                let mut a2 = frame.wrapping_sub(1800).wrapping_add(64);
                if a2 > 1024 {
                    a2 = 1024;
                }
                let scale = i32::from(a2);
                self.oxb = (i32::from(sin1024(a2, 6).wrapping_neg()) * scale / 40) as i16;
                self.oyb = (i32::from(sin1024(a2, 7).wrapping_neg()) * scale / 40) as i16;
                self.ozb = (i32::from(sin1024(a2, 8).wrapping_add(128)) * scale / 40) as i16;
            } else {
                // `else { oxb=-sin1024[(a*6)&1023]; … ozb=sin1024[(a*8)&1023]+128; }`
                // (`:467-472`), with the `a` that is `frame-900`.
                self.oxb = sin1024(a, 6).wrapping_neg();
                self.oyb = sin1024(a, 7).wrapping_neg();
                self.ozb = sin1024(a, 8).wrapping_add(128);
            }
            // `b=1800-frame; if(b<0) { if(b<-99) b=-99; oyp-=b*b/2; }`
            // (`:473-478`). `oyp` is 16-bit and this accumulates a wrap
            // every few frames after 1800.
            let mut b2 = 1800i16.wrapping_sub(frame);
            if b2 < 0 {
                if b2 < -99 {
                    b2 = -99;
                }
                self.oyp = self.oyp.wrapping_sub(div16(mul16(b2, b2), 2));
            }
        }

        if frame > 800 {
            // `MAIN.C:481-501`. The `else if(frame>1400+789)` arm is dead in
            // the source — `frame>1220+789` has already caught every value
            // that reaches it — and is kept here in the same order.
            if frame > 1220 + 789 {
                if self.xscale > 0 {
                    self.xscale -= 1;
                }
                if self.yscale > 0 {
                    self.yscale -= 1;
                }
                if self.zscale > 0 {
                    self.zscale -= 1;
                }
                if self.bscale > 0 {
                    self.bscale -= 1;
                }
            } else if frame > 1400 + 789 {
                if self.bscale > 0 {
                    self.bscale -= 8;
                }
                if self.bscale < 0 {
                    self.bscale = 0;
                }
            } else {
                if self.bscale < 180 {
                    self.bscale += 2;
                } else {
                    self.bscale = 180;
                }
            }
            if self.bscale > self.xscale {
                self.lightshift = 10;
            }
        } else {
            // `MAIN.C:502-536`, the bounce and the jello.
            if frame < 640 + 70 {
                self.yposa = self.yposa.wrapping_add(31);
                self.ypos = self.ypos.wrapping_add(div16(self.yposa, 40));
                if self.ypos > -300 {
                    self.ypos = self.ypos.wrapping_sub(div16(self.yposa, 40));
                    // `-yposa*boingm/boingd` truncates the product to 16
                    // bits before the divide (see the module docs).
                    self.yposa = div16(mul16(self.yposa.wrapping_neg(), self.boingm), self.boingd);
                    self.boingm = self.boingm.wrapping_add(2);
                    self.boingd = self.boingd.wrapping_add(1);
                }
                if self.ypos > -900 && self.yposa > 0 {
                    self.jello = div16(mul16(self.ypos.wrapping_add(900), 5), 3);
                    self.jelloa = 0;
                }
            } else if self.ypos > -2800 {
                self.ypos = self.ypos.wrapping_sub(16);
            } else if self.ypos < -2800 {
                self.ypos = self.ypos.wrapping_add(16);
            }
            let jello_over_30 = div16(self.jello, 30);
            self.xscale = 120i16.wrapping_add(jello_over_30);
            self.yscale = self.xscale;
            self.zscale = 120i16.wrapping_sub(jello_over_30);
            let previous = self.jello;
            self.jello = self.jello.wrapping_add(self.jelloa);
            if (previous < 0 && self.jello > 0) || (previous > 0 && self.jello < 0) {
                self.jelloa = div16(mul16(self.jelloa, 5), 6);
            }
            let a = div16(self.jello, 20);
            self.jelloa = self.jelloa.wrapping_sub(a);
        }

        // `MAIN.C:538-590`, the palette phases. The band clear writes both
        // buffers before the frame is painted (`:552-556`), which is the
        // order Task 5's note records.
        if frame > 1280 + 789 {
            let mut b = 1280i16.wrapping_add(789 + 64).wrapping_sub(frame);
            if b < 0 {
                b = 0;
            }
            self.fade_pal(b);
        } else if frame > 700 {
            if frame < 765 {
                let mut b = 764i16.wrapping_sub(frame);
                if b < 0 {
                    b = 0;
                }
                self.fade_pal(b);
            } else if frame < 790 {
                // `y=150+(frame-765)*2; memset(bgpic+y*320,0,640); …`
                let y = 150 + (frame - 765) * 2;
                let start = y as usize * VIEW_WIDTH as usize;
                bgpic[start..start + 2 * VIEW_WIDTH as usize].fill(0);
                vram[start..start + 2 * VIEW_WIDTH as usize].fill(0);
                if frame > 785 {
                    for index in 0..16i16 {
                        self.backpal[index as usize] = rebuilt_backpal(index);
                    }
                }
            } else if frame < 795 {
                self.pal = self.backpal;
            }
        }
    }

    /// `for(a=0;a<16*3;a++) pal[a]=backpal[a]*b/64;` (`MAIN.C:542`, `:550`).
    /// The source reads `backpal` and writes `pal`, so successive fades never
    /// compound.
    fn fade_pal(&mut self, b: i16) {
        for (entry, back) in self.pal.iter_mut().zip(self.backpal.iter()) {
            for (out_channel, &back_channel) in entry.iter_mut().zip(back.iter()) {
                *out_channel = ((i16::from(back_channel) * b) / 64) as u8;
            }
        }
    }
}

/// The frame-786 `backpal` rebuild (`MAIN.C:557-584`): the red ramp and the
/// grey-blue bit, clamped at 63.
fn rebuilt_backpal(a: i16) -> [u8; 3] {
    let (mut r, mut g, mut b) = (0i16, 0i16, 0i16);
    if a & 1 != 0 {
        r += 10;
    }
    if a & 2 != 0 {
        r += 30;
    }
    if a & 4 != 0 {
        r += 20;
    }
    if a & 8 != 0 {
        r += 16;
        g += 16;
        b += 16;
    }
    [r.min(63) as u8, g.min(63) as u8, b.min(63) as u8]
}

/// The `tmppal` build (`MAIN.C:365-390`): 256 entries — `a<16 ? a : a&7`
/// selects a `backpal` entry, and `a&8` above 15 lifts every channel by 16,
/// clamped to 63.
fn build_tmppal(backpal: &[[u8; 3]; 16]) -> Palette {
    let mut palette = Palette::new();
    for a in 0..256usize {
        let b = if a < 16 { a } else { a & 7 };
        let mut rgb = backpal[b];
        if a & 8 != 0 && a > 15 {
            for channel in &mut rgb {
                *channel = channel.saturating_add(16).min(63);
            }
        }
        palette.set_entry(a as u8, rgb);
    }
    palette
}

/// Where the part is in the source's sequence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Phase {
    /// `while(!dis_exit() && dis_musplus()<-19);` (`MAIN.C:290`).
    WaitMusic,
    /// `zoomer2()`'s 47 drawn bodies (`ZOOMER.C:38-69`).
    Zoomer,
    /// The FC bounce (`MAIN.C:322-350`).
    Bounce,
    /// `while(!dis_exit() && dis_getmframe()<300);` (`MAIN.C:353`).
    Wait300,
    /// The `dis_waitb` after that loop, whose post-actions build and upload
    /// `backpal`/`tmppal` (`MAIN.C:354-396`).
    Wait300Palette,
    /// The second `dis_waitb`, whose post-action snapshots `bgpic`
    /// (`MAIN.C:397-398`).
    Wait300Snapshot,
    /// `while(!dis_exit() && dis_getmframe()<333);` (`MAIN.C:400`).
    Wait333,
    /// The `dis_partstart()+dis_waitb()` tick (`MAIN.C:402-404`).
    Wait333Done,
    /// The main loop's first `dis_waitb` (`MAIN.C:413`): the wait consumes
    /// the tick and the frame's body is the next one.
    MainWait,
    /// One main-loop body a tick (`MAIN.C:417-641`).
    Main,
    /// Retired.
    Done,
}

/// GLENZ: the zoomer, the FC bounce, and the polyhedron over the picture.
pub struct Glenz {
    /// The 320x400 pixel buffer (`MAIN.C:18`), linear rows of 320.
    vram: Vec<u8>,
    /// The compositor's background (`MAIN.C:12`), 64,000 bytes.
    bgpic: Vec<u8>,
    /// `FC.UH`'s 320x200 pixels: `fcrow`/`fcrow2`'s source.
    fc: Vec<u8>,
    /// The picture's palette entries 0..15: `backpal` before the rebuild.
    picture_palette: [[u8; 3]; 16],
    main: MainState,
    colour: ColourState,
    compositor: Compositor,
    phase: Phase,
    zoomer: Option<Zoomer>,
    bounce: Option<Bounce>,
    /// `points2b`: the first rotation's output.
    rotated: Vec<[i32; 3]>,
    /// `points2`: the scaled/translated output and the clip's target.
    moved: Vec<[i32; 3]>,
    /// `points3`: the projected list.
    projected: Vec<PVert>,
}

impl Default for Glenz {
    fn default() -> Self {
        Self::new()
    }
}

impl Glenz {
    /// A fresh part: `init` reads `assets/GLENZ/FC.UH`.
    pub fn new() -> Glenz {
        Glenz {
            vram: vec![0; VRAM_PIXELS],
            bgpic: vec![0; SCREEN_PIXELS],
            fc: vec![0; SCREEN_PIXELS],
            picture_palette: [[0; 3]; 16],
            main: MainState::new([[0; 3]; 16]),
            colour: ColourState::new(),
            compositor: Compositor::new(),
            phase: Phase::WaitMusic,
            zoomer: None,
            bounce: None,
            rotated: Vec::new(),
            moved: Vec::new(),
            projected: Vec::new(),
        }
    }

    /// Copy `rows` 320-pixel rows of `vram` into the surface's first 320
    /// columns. The view reads the surface's own stride, so rows are written
    /// at the surface's width (640 in every run; `View::source_index`).
    fn blit(&self, ctx: &mut PartCtx, rows: usize) {
        let stride = ctx.surface.width();
        let width = VIEW_WIDTH as usize;
        let width = width.min(stride);
        let rows = rows.min(ctx.surface.height());
        let pixels = ctx.surface.pixels_mut();
        for y in 0..rows {
            let dst = y * stride;
            let src = y * VIEW_WIDTH as usize;
            pixels[dst..dst + width].copy_from_slice(&self.vram[src..src + width]);
        }
    }

    /// `int 10h` mode 13h and `testasm`'s live path (`MAIN.C:296-304`).
    ///
    /// The source's `memset(vram,0,65535)` runs in chain-4, so its 65,535
    /// addressed bytes cover all 32,000 bytes of a plane's 400 rows — every
    /// pixel of the 320x400 buffer. The epilogue has just zeroed it anyway;
    /// the clear is kept because the source performs it.
    ///
    /// The display is 320x200. `double_scan` is **false**: the emulator's
    /// mode-13h capture is the *halved* 320x200 frame (200 distinct source
    /// rows), which is what the reference files and what the bounce writes
    /// (`docs/decisions.md` D17's geometry note; `docs/fidelity/glenz.md`).
    /// The brief's `double_scan=true` describes the guest's VGA flag, not this
    /// engine's view, whose flag doubles source rows into a 400-line frame.
    fn mode_set(&mut self, ctx: &mut PartCtx) {
        self.vram.fill(0);
        *ctx.view = mode_view(VIEW_WIDTH, VIEW_HEIGHT_200, None, Some(0));
        // The bounce's DAC setup (`MAIN.C:315-318`): all 256 entries black,
        // then the picture's entries 0..15.
        ctx.set_palette(Palette::new());
        for (index, entry) in self.picture_palette.iter().enumerate() {
            ctx.set_palette_entry(index as u8, *entry);
        }
    }

    /// The copper's per-retrace upload, as the port models it: entries 0..15
    /// only (`MAIN.C:414-415`'s `16*3`), never `set_palette`.
    ///
    /// Called before [`MainState::step`], so the frame this tick composes
    /// pairs surface *N* with `pal` as it stood after frame *N−1* — the
    /// source's upload-before-work order (`MAIN.C:414-590`). The reference
    /// capture samples after the source's upload, so its DAC can lead its
    /// surface by one frame (D17; `docs/fidelity/glenz.md` §11).
    fn upload_low_palette(&self, ctx: &mut PartCtx) {
        for (index, entry) in self.main.pal.iter().enumerate() {
            ctx.set_palette_entry(index as u8, *entry);
        }
    }

    fn music_frame(ctx: &mut PartCtx) -> i32 {
        ctx.getmframe()
    }

    fn music_plus(ctx: &mut PartCtx) -> i32 {
        ctx.musplus()
    }

    /// `if(a<0 && a>-16) break;` (`MAIN.C:411`): MUSIC1's exit cue.
    fn is_music_cut(plus: i32) -> bool {
        plus < 0 && plus > -16
    }

    /// The main loop's two exit conditions plus the frame cap
    /// (`MAIN.C:409-411`).
    fn main_should_end(&self, ctx: &mut PartCtx) -> bool {
        if ctx.exit_requested {
            return true;
        }
        Self::is_music_cut(Self::music_plus(ctx)) || self.main.frame >= 7000
    }

    /// `MAIN.C:290-293`: wait for the music, then `dis_setmframe(0)` and
    /// `zoomer2()`'s first body *in the same tick*.
    ///
    /// The source's `while(!dis_exit() && dis_musplus()<-19);` exits on either
    /// condition and then **continues**: ESC does not skip `dis_setmframe(0)`
    /// or `zoomer2()` (`MAIN.C:291-293`). The zoomer checks only `kbhit`, so
    /// an exit flag runs all 47 of its bodies; the bounce's own
    /// `while(!dis_exit())` is what sees the flag next, and the main loop's
    /// check retires the part.
    fn wait_music(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if !ctx.exit_requested && Self::music_plus(ctx) < -19 {
            return PartStatus::Continuing;
        }
        if let Some(music) = ctx.music() {
            music.setmframe(0);
        }
        self.zoomer = Some(Zoomer::new(ctx.palette));
        self.phase = Phase::Zoomer;
        self.zoomer_frame(ctx)
    }

    /// One `zoomer2` body; the break tick runs the epilogue, the mode set and
    /// the bounce's first body (`ZOOMER.C:70-80`, `MAIN.C:296-350`).
    fn zoomer_frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let drew = {
            let zoomer = self.zoomer.as_mut().expect("the zoomer is running");
            zoomer.tick(&mut self.vram, ctx.palette)
        };
        if drew {
            self.blit(ctx, VIEW_HEIGHT_400 as usize);
            return PartStatus::Continuing;
        }
        intro::finish(&mut self.vram, ctx.palette);
        self.mode_set(ctx);
        self.bounce = Some(Bounce::new());
        self.phase = Phase::Bounce;
        self.bounce_frame(ctx)
    }

    /// One FC-bounce body (`MAIN.C:321-351`).
    ///
    /// The spring is `while(!dis_exit()) { ... }`: at the top of the body an
    /// exit flag skips the bounce entirely. The `getmframe` loop that follows
    /// is skipped for the same reason, and the unconditional `dis_waitb` is
    /// this tick — the palette build is the next one.
    fn bounce_frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.exit_requested {
            // `while(!dis_exit())` is not entered; the `getmframe` loop that
            // follows is skipped for the same flag; the unconditional
            // `dis_waitb` (`MAIN.C:354`) is this tick, so the palette build
            // is the next one.
            self.phase = Phase::Wait300Palette;
            return PartStatus::Continuing;
        }
        let drew = {
            let bounce = self.bounce.as_mut().expect("the bounce is running");
            bounce.tick(&mut self.vram, &self.fc)
        };
        if drew {
            self.blit(ctx, MAIN_ROWS);
            return PartStatus::Continuing;
        }
        self.phase = Phase::Wait300;
        if Self::music_frame(ctx) >= 300 {
            self.phase = Phase::Wait300Palette;
        }
        PartStatus::Continuing
    }

    /// `MAIN.C:355-396`: rebuild `backpal`, build and upload `tmppal`, set
    /// `lightshift` and zero the positions.
    fn wait300_palette(&mut self, ctx: &mut PartCtx) {
        self.main.backpal = self.picture_palette;
        let tmppal = build_tmppal(&self.main.backpal);
        ctx.set_palette(tmppal);
        self.main.reset_positions();
    }

    /// `MAIN.C:402-407`: `pal = backpal`, `dis_partstart()` and the DAC read
    /// back. The read returns exactly `backpal` because the last upload wrote
    /// `tmppal`'s entries 0..15 from it.
    fn part_start(&mut self) {
        self.main.pal = self.main.backpal;
        self.phase = Phase::MainWait;
    }

    /// The Part's `repeat` is one: the port's logic tick is one displayed
    /// frame, so the `while(repeat--)` body runs exactly once per call.
    ///
    /// The source's loop condition and music cut (`MAIN.C:409-411`) sit at the
    /// top of the iteration, which the body reaches *after* `cglenzdone` in
    /// the same frame: the frame is drawn, then the loop breaks. So this tick
    /// never checks before drawing; it draws and then retires the part on a
    /// cut, ESC or the frame-7000 cap. [`Phase::MainWait`] keeps the entry
    /// check, where a cut already visible breaks before the wait and the
    /// body, so that tick draws nothing.
    fn main_frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.upload_low_palette(ctx);
        self.main.step(&mut self.bgpic, &mut self.vram);
        self.colour.lightshift = self.main.lightshift;
        self.draw_objects(ctx);
        self.blit(ctx, MAIN_ROWS);
        if self.main_should_end(ctx) {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        PartStatus::Continuing
    }

    /// `cglenzinit` … `cglenzdone` (`MAIN.C:593-640`).
    fn draw_objects(&mut self, ctx: &mut PartCtx) {
        self.compositor.clear();
        if self.main.first_object_on() {
            // `MAIN.C:597-615`: demomode Glz, the rotation, the diagonal
            // scale with the translation, the clip while frame<800, the
            // projection and the point/colour list.
            let matrix = matrix_yxz(self.main.rx, self.main.ry, self.main.rz);
            rotate(&mut self.rotated, &POINTS, &matrix, [0, 0, 0]);
            let scale = Matrix([
                mul16(self.main.xscale, 64),
                0,
                0,
                0,
                mul16(self.main.yscale, 64),
                0,
                0,
                0,
                mul16(self.main.zscale, 64),
            ]);
            let add = [
                i32::from(self.main.oxp),
                i32::from(
                    self.main
                        .ypos
                        .wrapping_add(1500)
                        .wrapping_add(self.main.oyp),
                ),
                i32::from(ZPOS.wrapping_add(self.main.ozp)),
            ];
            rotate(&mut self.moved, &self.rotated, &scale, add);
            if self.main.frame < 800 {
                clip_y(&mut self.moved, 1500);
            }
            project(&mut self.projected, &self.moved, &INIT_320X200);
            let polys = build(
                EPOLYS,
                &self.projected,
                DemoMode::Glz,
                &mut self.colour,
                &self.main.backpal,
                ctx.palette,
            );
            for poly in &polys {
                self.compositor.add(poly);
            }
        }
        if self.main.second_object_on() {
            // `MAIN.C:620-637`: demomode Glz2, the reversed/third-speed
            // angles and the uniform bscale. The projection does not clip.
            let third = |value: i16| 3600i16.wrapping_sub(value / 3);
            let matrix = matrix_yxz(
                third(self.main.rx),
                third(self.main.ry),
                third(self.main.rz),
            );
            rotate(&mut self.rotated, &POINTSB, &matrix, [0, 0, 0]);
            let scale = mul16(self.main.bscale, 64);
            let matrix = Matrix([scale, 0, 0, 0, scale, 0, 0, 0, scale]);
            let add = [
                i32::from(self.main.oxb),
                i32::from(
                    self.main
                        .ypos
                        .wrapping_add(1500)
                        .wrapping_add(self.main.oyb),
                ),
                i32::from(ZPOS.wrapping_add(self.main.ozb)),
            ];
            rotate(&mut self.moved, &self.rotated, &matrix, add);
            project(&mut self.projected, &self.moved, &INIT_320X200);
            let polys = build(
                EPOLYSB,
                &self.projected,
                DemoMode::Glz2,
                &mut self.colour,
                &self.main.backpal,
                ctx.palette,
            );
            for poly in &polys {
                self.compositor.add(poly);
            }
        }
        // `cglenzdone` paints every span over `bgpic` into `vram`
        // (`MAIN.C:640`). Pixel 63999 is the compositor's sentinel and is
        // inherited, which is `bgpic`'s own byte.
        let Glenz {
            compositor,
            bgpic,
            vram,
            ..
        } = self;
        compositor.paint(bgpic, &mut vram[..SCREEN_PIXELS]);
    }
}

impl Part for Glenz {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "GLENZ waits on MUSIC1's musplus and the run has no music",
        )?;
        // The inherited screen is BEGLOGO's 320x400 Mode X. A run that starts
        // at GLENZ enters at whatever the shell's start view is; the part
        // installs the 400-row view itself, and the note records the
        // substitution.
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT_400;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = None;
        ctx.view.pan = 0;
        ctx.view.split = None;

        // The inherited picture: the surface's first 320 columns of rows
        // 0..400. A shorter surface leaves the rest black.
        let stride = ctx.surface.width();
        let width = (VIEW_WIDTH as usize).min(stride);
        let height = (VIEW_HEIGHT_400 as usize).min(ctx.surface.height());
        for y in 0..height {
            let src = y * stride;
            let dst = y * VIEW_WIDTH as usize;
            self.vram[dst..dst + width].copy_from_slice(&ctx.surface.pixels()[src..src + width]);
        }

        let path = asset(PICTURE);
        let image = load_image(NAME, PICTURE)?;
        if (image.width(), image.height()) != (VIEW_WIDTH, VIEW_HEIGHT_200) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{path}: {}x{}, expected 320x200",
                    image.width(),
                    image.height()
                ),
            });
        }
        self.fc.copy_from_slice(image.pixels());
        for (index, entry) in self.picture_palette.iter_mut().enumerate() {
            *entry = image.palette.entry(index as u8);
        }
        self.main = MainState::new(self.picture_palette);
        self.phase = Phase::WaitMusic;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match self.phase {
            Phase::WaitMusic => self.wait_music(ctx),
            Phase::Zoomer => self.zoomer_frame(ctx),
            Phase::Bounce => self.bounce_frame(ctx),
            Phase::Wait300 => {
                // `while(!dis_exit() && dis_getmframe()<300);`
                if ctx.exit_requested || Self::music_frame(ctx) >= 300 {
                    self.phase = Phase::Wait300Palette;
                }
                PartStatus::Continuing
            }
            Phase::Wait300Palette => {
                self.wait300_palette(ctx);
                self.phase = Phase::Wait300Snapshot;
                PartStatus::Continuing
            }
            Phase::Wait300Snapshot => {
                // `memcpy(bgpic,vram,64000);` (`MAIN.C:398`), then the
                // `getmframe()<333` loop starts in the same interval.
                self.bgpic.copy_from_slice(&self.vram[..SCREEN_PIXELS]);
                self.phase = Phase::Wait333;
                self.poll_333(ctx);
                PartStatus::Continuing
            }
            Phase::Wait333 => {
                self.poll_333(ctx);
                PartStatus::Continuing
            }
            Phase::Wait333Done => {
                self.part_start();
                PartStatus::Continuing
            }
            Phase::MainWait => {
                if self.main_should_end(ctx) {
                    self.phase = Phase::Done;
                    PartStatus::Finished
                } else {
                    self.phase = Phase::Main;
                    PartStatus::Continuing
                }
            }
            Phase::Main => self.main_frame(ctx),
            Phase::Done => PartStatus::Finished,
        }
    }
}

impl Glenz {
    /// `while(!dis_exit() && dis_getmframe()<333);` (`MAIN.C:400`): the
    /// source tests `dis_exit` here, so an exit flag ends the wait and lets
    /// the part-start tick and the main loop's own check retire it.
    fn poll_333(&mut self, ctx: &mut PartCtx) {
        if ctx.exit_requested || Self::music_frame(ctx) >= 333 {
            self.phase = Phase::Wait333Done;
        } else {
            self.phase = Phase::Wait333;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{Rig, ScriptedMusic};


    /// The scripted player: a `musplus` schedule by tick and a `getmframe`
    /// that advances at a fixed rate from the tick `setmframe(0)` ran on.
    ///
    /// The measured rate is MUSIC1's tempo-130 tick clock: 52 music ticks a
    /// second against the reference's 70.086 video frames, i.e.
    /// 52,000/70,086 ≈ 0.7419 zframes a frame.
    /// With that rate the reset tick is followed by zframe 300 at +404 frames
    /// and 333 at +449 — the measured 1515 and 1560 from 1111
    /// (`reference/music/second2b`; `reference-indices.md` §7.6).
    fn player(rate_num: i64, rate_den: i64, musplus: Vec<i32>) -> ScriptedMusic {
        assert!(!musplus.is_empty());
        ScriptedMusic::musplus(musplus).mframe_rate(rate_num, rate_den)
    }

    /// The measured clock and a gate that closes at `reset` (so the first
    /// `reset` calls see -32 and the call at `reset` sees -19).
    fn measured(reset: usize) -> ScriptedMusic {
        let mut schedule = vec![-32; reset];
        schedule.push(-19);
        player(52_000, 70_086, schedule)
    }

    /// A clock that crosses 300 and 333 on its first poll, so the
    /// intro's waits do not dominate a test that is not about them.
    fn fast() -> ScriptedMusic {
        player(1000, 1, vec![-19])
    }

    /// Set this tick's `musplus`, extending the schedule with the last
    /// value.
    fn cue(music: &mut ScriptedMusic, tick: usize, value: i32) {
        if music.script.len() <= tick {
            let last = *music.script.last().expect("a schedule");
            music.script.resize(tick + 1, last);
        }
        music.script[tick].musplus = value;
    }

    /// The tick the last `setmframe` ran on.
    fn reset_tick(music: &mut ScriptedMusic) -> Option<usize> {
        music.setmframe_calls.last().map(|&(tick, _)| tick)
    }

    /// One call of the part at `tick`. The part sets no copper, so a tick is
    /// its `frame` alone.
    trait Steps {
        fn step_at(&mut self, part: &mut Glenz, tick: u32) -> PartStatus;
        fn step_exit_at(&mut self, part: &mut Glenz, tick: u32) -> PartStatus;
    }

    impl Steps for Rig {
        fn step_at(&mut self, part: &mut Glenz, tick: u32) -> PartStatus {
            part.frame(&mut self.ctx(tick))
        }

        fn step_exit_at(&mut self, part: &mut Glenz, tick: u32) -> PartStatus {
            part.frame(&mut self.ctx_exit(tick))
        }
    }

    /// The part started on the rig's music, as the shell starts it.
    fn started(music: ScriptedMusic) -> (Rig, Glenz) {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT_200).with(music);
        let mut part = Glenz::new();
        part.init(&mut rig.ctx(0))
            .expect("the test music is installed");
        (rig, part)
    }

    /// `FC.UH`'s palette entries 0..15, the `backpal` source.
    fn picture_palette() -> [[u8; 3]; 16] {
        let image = load_image(NAME, PICTURE).unwrap();
        let mut palette = [[0u8; 3]; 16];
        for (index, entry) in palette.iter_mut().enumerate() {
            *entry = image.palette.entry(index as u8);
        }
        palette
    }

    /// Drive the intro with the fast clock until the main loop is reached.
    /// Returns the tick the first main body will occupy (the call is the
    /// MainWait transition, so the caller steps once for frame 1).
    fn to_main(rig: &mut Rig, part: &mut Glenz) -> u32 {
        for tick in 0..1000u32 {
            if part.phase == Phase::Main {
                return tick;
            }
            assert_eq!(rig.step_at(part, tick), PartStatus::Continuing);
        }
        panic!("the intro did not reach the main loop in 1,000 ticks");
    }

    /// Drive the intro with the fast clock until `Phase::MainWait` is
    /// reached. Returns the tick the MainWait call will occupy.
    fn to_main_wait(rig: &mut Rig, part: &mut Glenz) -> u32 {
        for tick in 0..1000u32 {
            if part.phase == Phase::MainWait {
                return tick;
            }
            assert_eq!(rig.step_at(part, tick), PartStatus::Continuing);
        }
        panic!("the intro did not reach the main wait in 1,000 ticks");
    }

    /// Pixels of the main frame's buffer that differ from `bgpic` above the
    /// floor: the object's drawn pixels at the frames before the band clear.
    fn object_pixels(part: &Glenz) -> usize {
        (0..150 * 320)
            .filter(|&at| part.vram[at] != part.bgpic[at])
            .count()
    }

    /// Drive the measured intro and return the phases' transition ticks —
    /// the tick of the call that made each transition.
    fn measured_intro(rig: &mut Rig, part: &mut Glenz) -> Vec<(Phase, u32)> {
        let mut transitions = Vec::new();
        for tick in 0..800u32 {
            if part.phase == Phase::Main {
                break;
            }
            let before = part.phase;
            assert_eq!(rig.step_at(part, tick), PartStatus::Continuing);
            if part.phase != before {
                transitions.push((part.phase, tick));
            }
        }
        transitions
    }

    /// The registry row and the part's name.
    #[test]
    fn glenz_is_registered() {
        assert_eq!(crate::parts::registered("glenz"), Some("glenz"));
        assert_eq!(Glenz::new().name(), "glenz");
    }

    /// A run without music is refused loudly: the part would wait for ever.
    #[test]
    fn init_refuses_a_run_without_music() {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT_200).with(fast());
        let mut part = Glenz::new();
        let error = part.init(&mut rig.ctx_without_music(0)).unwrap_err();
        assert_eq!(error.part, "glenz");
        assert!(error.message.contains("no music"), "{}", error.message);
        assert_eq!(part.phase, Phase::WaitMusic, "nothing has started");
    }

    /// Init installs BEGLOGO's inherited 320x400 view and copies the
    /// surface's first 320 columns of rows 0..400 into `vram`.
    #[test]
    fn init_takes_the_inherited_400_row_screen() {
        let mut rig = Rig::new(VIEW_WIDTH, VIEW_HEIGHT_200).with(fast());
        for (index, pixel) in rig.surface.pixels_mut().iter_mut().enumerate() {
            *pixel = (index % 251) as u8;
        }
        let mut part = Glenz::new();
        part.init(&mut rig.ctx(0)).unwrap();
        assert_eq!(
            (rig.view.width, rig.view.height),
            (VIEW_WIDTH, VIEW_HEIGHT_400)
        );
        assert!(!rig.view.double_scan);
        assert_eq!(rig.view.start_addr, 0);
        assert_eq!(rig.view.y_line_offset, None);
        for y in 0..400usize {
            for x in 0..320usize {
                assert_eq!(
                    part.vram[y * 320 + x],
                    rig.surface.pixels()[y * 640 + x],
                    "({x},{y})"
                );
            }
        }
        // The zoomer's first draw fills row 0 white and blits all 400 rows.
        rig.step_at(&mut part, 0);
        assert_eq!(&rig.surface.row(0)[..320], &[255u8; 320]);
        assert_eq!(
            rig.surface.row(399)[319],
            255,
            "the bottom band's first row"
        );
    }

    /// The zoomer's first body is the music wait's exit tick, and the reset
    /// is `dis_setmframe(0)` in that same call (`MAIN.C:290-293`).
    #[test]
    fn the_music_wait_exits_into_the_zoomers_first_tick() {
        let (mut rig, mut part) = started(measured(4));
        let mut tick = 0u32;
        for _ in 0..4 {
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            assert_eq!(part.phase, Phase::WaitMusic);
            tick += 1;
        }
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Zoomer, "the exit call draws");
        assert_eq!(reset_tick(rig.player()), Some(4));
        assert_eq!(
            rig.player().setmframe_calls,
            vec![(4, 0)],
            "one reset, at the wait's exit"
        );
    }

    /// The tick map against the measured anchors (D37): 47 zoomer draws,
    /// 177 bounce draws, `getmframe` 300 at +404 and 333 at +449, the three
    /// `dis_waitb` ticks and the main loop's first wait at +451.
    #[test]
    fn the_intro_is_47_plus_177_ticks_and_the_measured_waits() {
        let (mut rig, mut part) = started(measured(4));
        let transitions = measured_intro(&mut rig, &mut part);
        assert_eq!(reset_tick(rig.player()), Some(4));

        let at = |phase: Phase| {
            transitions
                .iter()
                .find(|(seen, _)| *seen == phase)
                .map(|(_, tick)| *tick)
                .unwrap_or_else(|| panic!("no transition to {phase:?}: {transitions:?}"))
        };

        // The reset is tick 4; the 47th zoomer draw is tick 50, the break
        // (which draws the bounce's first band) is tick 51.
        assert_eq!(part.bounce.as_ref().map(|b| b.drawn()), Some(177));
        assert_eq!(part.bounce.as_ref().unwrap().band(), (153, 200));

        // `getmframe()<300` first fails at 1111 + 404 = 1515.
        let wait300 = at(Phase::Wait300Palette);
        assert_eq!(wait300, 4u32 + 404, "zframe 300 first read at +404");
        // The palette build, the snapshot, then `getmframe()<333` at +449.
        assert_eq!(at(Phase::Wait300Snapshot), 4u32 + 405);
        assert_eq!(at(Phase::Wait333), 4u32 + 406);
        assert_eq!(
            at(Phase::Wait333Done),
            4u32 + 449,
            "zframe 333 first read at +449"
        );
        // The part-start wait and the main loop's first wait: +450, +451.
        assert_eq!(at(Phase::MainWait), 4u32 + 450);
        assert_eq!(at(Phase::Main), 4u32 + 451);

        // The main body then draws frame 1 at +452 — the origin the object
        // evidence needs (main frame 44 at reference 1606 = 1562 + 44).
        assert_eq!(rig.step_at(&mut part, 4 + 452), PartStatus::Continuing);
        assert_eq!(part.main.frame, 1);
    }

    /// The mode set: 320x200, origin 0, no line offset, and the bounce's
    /// all-black DAC with the picture's entries 0..15.
    #[test]
    fn the_mode_set_installs_mode_13h_and_the_picture_palette() {
        let (mut rig, mut part) = started(fast());
        let mut tick = 0;
        while part.phase != Phase::Bounce {
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            tick += 1;
        }
        assert_eq!((rig.view.width, rig.view.height), (320, 200));
        assert!(!rig.view.double_scan);
        assert_eq!(rig.view.start_addr, 0);
        assert_eq!(rig.view.y_line_offset, None);
        assert_eq!(rig.view.border, Some(0));
        for index in 0..16u8 {
            assert_eq!(rig.palette.entry(index), picture_palette()[index as usize]);
        }
        for index in 16..=255u8 {
            assert_eq!(rig.palette.entry(index), [0, 0, 0], "entry {index}");
        }
    }

    /// `MAIN.C:365-390`: the `tmppal` upload is all 256 entries, and its
    /// entries 0..15 are `backpal` exactly.
    #[test]
    fn the_wait300_tick_uploads_tmppal() {
        let (mut rig, mut part) = started(fast());
        let mut tick = 0;
        while part.phase != Phase::Wait300Palette {
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            tick += 1;
        }
        // This tick is the palette build.
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
        let backpal = picture_palette();
        for a in 0..256usize {
            let b = if a < 16 { a } else { a & 7 };
            let mut expected = backpal[b];
            if a & 8 != 0 && a > 15 {
                expected = expected.map(|channel| channel.saturating_add(16).min(63));
            }
            assert_eq!(rig.palette.entry(a as u8), expected, "tmppal[{a}]");
        }
        assert_eq!(
            rig.palette.entry(0),
            backpal[0],
            "the DAC read back at part start is backpal"
        );
    }

    /// After the intro, the main loop draws exactly one frame a tick.
    #[test]
    fn the_main_loop_advances_one_frame_a_tick() {
        let (mut rig, mut part) = started(fast());
        let start = to_main(&mut rig, &mut part);
        for (offset, expected) in (1..=10i16).enumerate() {
            assert_eq!(part.phase, Phase::Main);
            let tick = start + offset as u32;
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            assert_eq!(part.main.frame, expected);
        }
    }

    /// The first main frame is `bgpic` — the object is off-screen until frame
    /// 44. Pixel 63999 is the compositor's sentinel and is never painted; it
    /// is `bgpic`'s own byte.
    #[test]
    fn the_first_main_frame_is_the_background() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main(&mut rig, &mut part);
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
        assert_eq!(part.main.frame, 1);
        assert_eq!(part.vram[..SCREEN_PIXELS], part.bgpic[..]);
    }

    /// The measured corpus: the first object pixel is main frame 44, and the
    /// six silhouettes at 1606..1611 are frames 44..49 (`tools/glenz_physics.py
    /// entry`; `reference-indices.md` §7.6). The boxes below are that tool's,
    /// against the frame's own sky; `bgpic`'s rows 0..149 are the same sky.
    #[test]
    fn the_first_object_appears_on_frame_44_with_the_measured_silhouettes() {
        const EXPECTED: &[(i32, i32, i32, i32, usize, f64, f64)] = &[
            (172, 179, 0, 0, 8, 175.5, 0.0),
            (166, 179, 0, 1, 21, 173.3, 0.3),
            (157, 180, 0, 3, 60, 170.5, 1.0),
            (153, 180, 0, 4, 86, 168.8, 1.4),
            (149, 179, 0, 5, 111, 166.6, 1.7),
            (146, 180, 0, 6, 137, 165.5, 2.0),
        ];
        let (mut rig, mut part) = started(fast());
        let start = to_main(&mut rig, &mut part);
        let mut first = None;
        for (offset, frame) in (1..=49i16).enumerate() {
            assert_eq!(
                rig.step_at(&mut part, start + offset as u32),
                PartStatus::Continuing
            );
            assert_eq!(part.main.frame, frame);
            let mut points = Vec::new();
            for y in 0..150usize {
                for x in 0..320usize {
                    let at = y * 320 + x;
                    // Only the object can differ in these rows: the sky is
                    // uniform there until the frame-765 band clear.
                    if part.vram[at] != part.bgpic[at] {
                        points.push((x as i32, y as i32));
                    }
                }
            }
            if !points.is_empty() && first.is_none() {
                first = Some(frame);
            }
            if (44..=49).contains(&frame) {
                let entry = EXPECTED[(frame - 44) as usize];
                let expected = (
                    entry.0, entry.1, entry.2, entry.3, entry.4, entry.5, entry.6,
                );
                let xs: Vec<i32> = points.iter().map(|p| p.0).collect();
                let ys: Vec<i32> = points.iter().map(|p| p.1).collect();
                assert_eq!(
                    (
                        *xs.iter().min().unwrap(),
                        *xs.iter().max().unwrap(),
                        *ys.iter().min().unwrap(),
                        *ys.iter().max().unwrap(),
                        points.len(),
                    ),
                    (expected.0, expected.1, expected.2, expected.3, expected.4),
                    "frame {frame}'s box"
                );
                let cx = xs.iter().sum::<i32>() as f64 / points.len() as f64;
                let cy = ys.iter().sum::<i32>() as f64 / points.len() as f64;
                assert!(
                    (cx - expected.5).abs() < 0.05,
                    "frame {frame} centroid x {cx}"
                );
                assert!(
                    (cy - expected.6).abs() < 0.05,
                    "frame {frame} centroid y {cy}"
                );
            }
        }
        assert_eq!(first, Some(44), "the first object pixels are main frame 44");
        // 1606 = 1562 + 44 is the reference index of frame 44.
        assert_eq!(1562 + first.unwrap(), 1606);
    }

    /// The palette phases of `MAIN.C:538-590`, on the physics alone.
    #[test]
    fn the_palette_phases_have_the_source_boundaries() {
        let backpal = picture_palette();
        let mut state = MainState::new(backpal);
        let mut bgpic = vec![0xFFu8; SCREEN_PIXELS];
        let mut vram = vec![0xFFu8; SCREEN_PIXELS];
        let mut pal_at = Vec::new();
        for frame in 1..=2133i16 {
            state.step(&mut bgpic, &mut vram);
            assert_eq!(state.frame, frame);
            if matches!(
                frame,
                700 | 701 | 764 | 765 | 785 | 786 | 790 | 795 | 2069 | 2070 | 2133
            ) {
                pal_at.push((frame, state.pal));
            }
        }
        let get = |frame: i16| {
            pal_at
                .iter()
                .find(|(at, _)| *at == frame)
                .map(|(_, pal)| *pal)
                .unwrap()
        };
        // Through frame 700 the fade branch has not run.
        assert_eq!(get(700), backpal);
        // `b=764-frame`: at 701 b=63, at 764 b=0.
        let faded = |b: i16| backpal.map(|rgb| rgb.map(|c| ((i16::from(c) * b) / 64) as u8));
        assert_eq!(get(701), faded(63));
        assert_eq!(get(764), faded(0));
        // 765..789: `pal` stays at 764's black; the band clear runs.
        assert_eq!(get(765), [[0u8; 3]; 16]);
        assert_eq!(get(785), [[0u8; 3]; 16]);
        // 786..789 rebuild backpal, but not pal.
        assert_eq!(get(786), [[0u8; 3]; 16]);
        let rebuilt: [[u8; 3]; 16] = std::array::from_fn(|a| rebuilt_backpal(a as i16));
        assert_eq!(state.backpal, rebuilt);
        // 790 copies backpal into pal.
        assert_eq!(get(790), rebuilt);
        assert_eq!(get(795), rebuilt);
        assert_eq!(get(2069), rebuilt);
        // The second fade starts at 2070 (b=63) and ends at 2133 (b=0).
        assert_eq!(
            get(2070),
            rebuilt.map(|rgb| rgb.map(|c| ((i16::from(c) * 63) / 64) as u8))
        );
        assert_eq!(get(2133), [[0u8; 3]; 16]);

        // The band clear: two rows a frame from row 150, both buffers, and
        // nothing after frame 789 (`MAIN.C:552-556`).
        let mut state = MainState::new(backpal);
        let mut bgpic = vec![0xFFu8; SCREEN_PIXELS];
        let mut vram = vec![0xFFu8; SCREEN_PIXELS];
        for _ in 1..=789i16 {
            state.step(&mut bgpic, &mut vram);
        }
        for row in 150..200usize {
            assert!(
                bgpic[row * 320..(row + 1) * 320].iter().all(|&p| p == 0),
                "bgpic row {row}"
            );
            assert!(
                vram[row * 320..(row + 1) * 320].iter().all(|&p| p == 0),
                "vram row {row}"
            );
        }
        assert!(
            bgpic[..150 * 320].iter().all(|&p| p == 0xFF),
            "above the band is untouched"
        );
    }

    /// The dynamic band's last clear is rows 198/199 at frame 789; the clear
    /// happens *before* the paint, which is why the background snapshot sees
    /// it (`MAIN.C:552-556` precedes `cglenzdone`).
    #[test]
    fn the_band_clear_is_at_rows_150_onwards_and_stops_at_789() {
        let backpal = picture_palette();
        let mut state = MainState::new(backpal);
        let mut bgpic = vec![9u8; SCREEN_PIXELS];
        let mut vram = vec![9u8; SCREEN_PIXELS];
        for _ in 1..=765i16 {
            state.step(&mut bgpic, &mut vram);
        }
        assert!(bgpic[150 * 320..152 * 320].iter().all(|&p| p == 0));
        assert!(bgpic[152 * 320..153 * 320].iter().all(|&p| p == 9));
        for _ in 766..=789i16 {
            state.step(&mut bgpic, &mut vram);
        }
        assert!(bgpic[198 * 320..200 * 320].iter().all(|&p| p == 0));
        state.step(&mut bgpic, &mut vram);
        assert_eq!(state.frame, 790);
        assert!(bgpic[150 * 320..200 * 320].iter().all(|&p| p == 0));
        assert!(
            bgpic[..150 * 320].iter().all(|&p| p == 9),
            "no clear outside the band, and none after 789"
        );
    }

    /// The object switches: the first while `xscale>4`, the second from
    /// frame 803 to 2185 (`bscale` growing at 2 a frame from 801, shrinking
    /// from 2010).
    #[test]
    fn the_object_switches_follow_the_scales() {
        let backpal = picture_palette();
        let mut state = MainState::new(backpal);
        let mut bgpic = vec![0u8; SCREEN_PIXELS];
        let mut vram = vec![0u8; SCREEN_PIXELS];
        let mut first_off = None;
        let mut second_on = None;
        let mut second_off = None;
        let mut lightshift_10 = None;
        let mut xscale_800 = None;
        for frame in 1..=2200i16 {
            state.step(&mut bgpic, &mut vram);
            if frame == 800 {
                xscale_800 = Some(state.xscale);
            }
            if !state.first_object_on() && first_off.is_none() {
                first_off = Some(frame);
            }
            if state.second_object_on() && second_on.is_none() {
                second_on = Some(frame);
            }
            if second_on.is_some() && !state.second_object_on() && second_off.is_none() {
                second_off = Some(frame);
            }
            if state.lightshift == 10 && lightshift_10.is_none() {
                lightshift_10 = Some(frame);
            }
        }
        assert_eq!(second_on, Some(803), "bscale 2, 4, then 6");
        assert_eq!(second_off, Some(2185), "bscale falls to 4");
        assert_eq!(first_off, Some(2120), "xscale falls to 4");
        assert_eq!(lightshift_10, Some(858), "bscale overtakes xscale");
        // The 16-bit bounce product has already wrapped by here: the model
        // with unbounded `int`s reaches a different scale.
        assert_eq!(xscale_800, Some(115));
    }

    /// The second object's draw path is live from frame 803: the same first
    /// object and a state differing only in `bscale` (4 → 6) paint different
    /// pictures, so `demo_glz2`'s reversed angles and uniform scale reach the
    /// compositor.
    #[test]
    fn the_second_object_draws_when_bscale_exceeds_4() {
        fn force(part: &mut Glenz, frame: i16, bscale: i16) {
            part.main.frame = frame;
            part.main.bscale = bscale;
            part.main.xscale = 120;
            part.main.yscale = 120;
            part.main.zscale = 120;
            part.main.rx = 1600;
            part.main.ry = 350;
            part.main.rz = 0;
            part.main.ypos = -2799;
            part.main.oxp = 0;
            part.main.oyp = 0;
            part.main.ozp = 0;
            part.main.oxb = 0;
            part.main.oyb = 0;
            part.main.ozb = 0;
        }
        let (mut rig, mut part) = started(fast());
        let tick = to_main(&mut rig, &mut part);
        force(&mut part, 802, 4);
        {
            let mut ctx = rig.ctx(tick);
            part.draw_objects(&mut ctx);
        }
        let without = part.vram.clone();
        force(&mut part, 803, 6);
        {
            let mut ctx = rig.ctx(tick);
            part.draw_objects(&mut ctx);
        }
        assert!(
            part.vram != without,
            "bscale 6 from frame 803 must add the second object's pixels"
        );
    }

    /// A cut present at the loop's entry (`Phase::MainWait`) breaks before the
    /// wait and the body, so that tick draws nothing (`MAIN.C:409-411`).
    #[test]
    fn the_music_cut_at_the_main_wait_draws_nothing() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main_wait(&mut rig, &mut part);
        let before = part.vram.clone();
        let surface_before = rig.surface.pixels().to_vec();
        // -1 is inside (-16, 0): the cue.
        cue(rig.player(), tick as usize, -1);
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.main.frame, 0, "the entry check draws no frame");
        assert_eq!(part.vram, before, "the buffer is untouched");
        assert_eq!(
            rig.surface.pixels(),
            &surface_before[..],
            "the screen holds"
        );
        // The cue's boundaries: -1..-15 cut; -16, 0 and anything positive or
        // the -32 sentinel do not.
        assert!(Glenz::is_music_cut(-1));
        assert!(Glenz::is_music_cut(-15));
        for outside in [-16, 0, 1, -32] {
            assert!(!Glenz::is_music_cut(outside), "{outside}");
        }
    }

    /// A cut first visible on a body tick draws that frame and retires the
    /// part on the same tick: the source's loop top (`MAIN.C:409-411`) is
    /// reached after `cglenzdone`, so the frame is kept. Frame 44 is the
    /// object's first drawn frame, which makes "drawn" visible.
    #[test]
    fn the_music_cut_on_a_body_tick_draws_then_finishes() {
        let (mut rig, mut part) = started(fast());
        let start = to_main(&mut rig, &mut part);
        for frame in 1..44i16 {
            let tick = start + frame as u32 - 1;
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            assert_eq!(part.main.frame, frame);
        }
        let tick = start + 43;
        cue(rig.player(), tick as usize, -1);
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.main.frame, 44, "the cut tick's frame is drawn");
        assert!(
            object_pixels(&part) > 0,
            "frame 44's object is on the screen"
        );
    }

    /// ESC at the loop's entry draws nothing, and the part asks for no drain.
    #[test]
    fn escape_at_the_main_wait_finishes_without_a_drain() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main_wait(&mut rig, &mut part);
        let before = part.vram.clone();
        assert!(!part.drains_after_exit());
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.main.frame, 0, "ESC at the entry draws no frame");
        assert_eq!(part.vram, before, "ESC draws nothing on its tick");
    }

    /// ESC first visible on a body tick draws that frame and retires the part
    /// on the same tick, with no drain.
    #[test]
    fn escape_on_a_body_tick_draws_then_finishes() {
        let (mut rig, mut part) = started(fast());
        let start = to_main(&mut rig, &mut part);
        for frame in 1..44i16 {
            let tick = start + frame as u32 - 1;
            assert_eq!(rig.step_at(&mut part, tick), PartStatus::Continuing);
            assert_eq!(part.main.frame, frame);
        }
        let tick = start + 43;
        assert!(!part.drains_after_exit());
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.main.frame, 44, "the ESC tick's frame is drawn");
        assert!(
            object_pixels(&part) > 0,
            "frame 44's object is on the screen"
        );
    }

    /// `frame < 7000` is the loop condition checked at the same top after the
    /// body: frame 7000 is drawn and the part retires on that tick.
    #[test]
    fn the_frame_cap_draws_7000_then_finishes() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main(&mut rig, &mut part);
        part.main.frame = 6999;
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.main.frame, 7000, "frame 7000 is drawn before the cap");
    }

    /// The cap already reached at the loop's entry breaks without a body.
    #[test]
    fn the_frame_cap_at_the_main_wait_draws_nothing() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main_wait(&mut rig, &mut part);
        part.main.frame = 7000;
        let before = part.vram.clone();
        assert_eq!(rig.step_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.main.frame, 7000);
        assert_eq!(part.vram, before, "no body runs");
    }

    /// ESC during the music wait does **not** retire the part: `MAIN.C:290`'s
    /// `while(!dis_exit() && musplus<-19);` exits and the source continues to
    /// `dis_setmframe(0)` and `zoomer2()`. The zoomer checks only `kbhit`, so
    /// its 47 bodies run; the bounce's `while(!dis_exit())` skips every body;
    /// the waits fall through; the main loop's check retires the part.
    #[test]
    fn escape_during_the_music_wait_continues_into_the_zoomer() {
        let (mut rig, mut part) = started(measured(4));
        assert_eq!(rig.step_at(&mut part, 0), PartStatus::Continuing);
        // musplus is still -32, but the exit flag ends the gate anyway.
        assert_eq!(rig.step_exit_at(&mut part, 1), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Zoomer, "the source still calls zoomer2");
        assert_eq!(
            reset_tick(rig.player()),
            Some(1),
            "dis_setmframe(0) still runs"
        );

        let mut tick = 2;
        for _ in 1..47 {
            assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Continuing);
            tick += 1;
        }
        assert_eq!(part.zoomer.as_ref().unwrap().zy(), 260);
        // The break tick: epilogue and mode set, then the bounce's loop is
        // skipped and the palette-build tick follows.
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Continuing);
        tick += 1;
        assert_eq!(part.phase, Phase::Wait300Palette);
        assert_eq!(
            part.bounce.as_ref().map(|b| b.drawn()),
            Some(0),
            "no bounce body draws on ESC"
        );
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Continuing); // palette
        tick += 1;
        assert_eq!(part.phase, Phase::Wait300Snapshot);
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Continuing); // bgpic
        tick += 1;
        assert_eq!(part.phase, Phase::Wait333Done);
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Continuing); // part start
        tick += 1;
        assert_eq!(part.phase, Phase::MainWait);
        assert_eq!(rig.step_exit_at(&mut part, tick), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
    }

    /// The per-tick upload writes entries 0..15 only: a group's entries
    /// 16..255 survive it (`MAIN.C:414-415`'s `16*3`).
    #[test]
    fn the_low_palette_upload_never_touches_the_groups() {
        let (mut rig, mut part) = started(fast());
        let tick = to_main(&mut rig, &mut part);
        // A sentinel in every group entry.
        for index in 16..=255u8 {
            rig.palette.set_entry(index, [1, 2, 3]);
        }
        part.main.pal = [[9, 8, 7]; 16];
        {
            let mut ctx = rig.ctx(tick);
            part.upload_low_palette(&mut ctx);
        }
        for index in 0..16u8 {
            assert_eq!(rig.palette.entry(index), [9, 8, 7], "entry {index}");
        }
        for index in 16..=255u8 {
            assert_eq!(rig.palette.entry(index), [1, 2, 3], "entry {index}");
        }
    }

    /// `mul16`/`div16` are the source's 16-bit int arithmetic; the values
    /// that matter are the ones the animation reaches.
    #[test]
    fn the_sixteen_bit_products_wrap_like_the_source() {
        // Frame 667: `-yposa*boingm` = -40160 wraps to 25376 before the
        // divide by boingd (9) — the source's `imul` keeps only AX.
        assert_eq!(mul16(-4016, 10), 25376);
        assert_eq!(div16(mul16(-4016, 10), 9), 25376 / 9);
        // `oyp -= b*b/2` with b=-99 accumulates; the subtraction wraps at 16
        // bits.
        let mut oyp = 1000i16;
        oyp = oyp.wrapping_sub(div16(mul16(-99, -99), 2));
        assert_eq!(oyp, 1000 - 4900);
        // The orbit products fit; their division truncates toward zero.
        assert_eq!(div16(-7, 10), 0);
        assert_eq!(div16(7, 10), 0);
        assert_eq!(div16(-11, 10), -1);
    }

    /// The source's `sin1024` indexing: `(a*n)&1023` on the 16-bit product's
    /// low word, so a product past 32,767 still masks correctly.
    #[test]
    fn the_sin1024_index_wraps_at_1024() {
        let a = 6100i16;
        assert_eq!(
            sin1024(a, 6),
            SIN1024[(a.wrapping_mul(6) as u16 & 1023) as usize]
        );
        assert_eq!(sin1024(0, 3), 0);
        assert_eq!(sin1024(256, 1), 256);
        assert_eq!(sin1024(768, 1), -256);
        assert_eq!(sin1024(1024, 1), 0, "& 1023 wraps");
    }
}
