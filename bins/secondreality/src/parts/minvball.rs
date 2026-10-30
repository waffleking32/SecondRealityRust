//! MINVBALL — the mini vector balls: 512 dots over a grey floor.
//!
//! `DOTS/MAIN.C` (291 lines) is the part; `DOTS/ASM.ASM`'s `drawdots`
//! (`:52-218`) projects and draws the dots, and both read `SIN1024.INC`
//! ([`SIN1024`]). The RNG is Microsoft C 6.0's `rand()` ([`MsRand`], never
//! seeded), not Turbo C's (`docs/fidelity/minvball.md` §2). A tick is one
//! `dis_waitb()`:
//!
//! ```text
//! tick 0           :79-167   the shuffles, the palette built, read back and
//!                            zeroed (:151), the floor ramp into bgpic, the depth tables
//! ticks 1-129 odd  :171-183  the 65 fade uploads, pal - b for b = 64 down to 0
//! tick 130 on      :185-284  the main loop from frame 1: pal2 once frame > 2300,
//!                            the cue, the stage physics, drawdots; frame < 2450
//! ```
//!
//! The source advances the physics `repeat = dis_waitb()` times an iteration;
//! the port once a tick, which the keyframes measure as exact (note §3). The cue
//! (`:191-195`, `-4 < dis_musplus() < 0`) sits inside `dis_indemo()`; the port
//! is always in demo, so a standalone run keeps it. In the reference run the
//! frame cap comes first (note §5). Tick 0 is boot 10,368, a mapping and not an
//! anchor (note §3), so `init` makes the music's head start itself.
//!
//! Preserved bugs (`bugs.md` §2, §18):
//! - the dots paint in array order with no depth test ([`drawdots`]);
//! - `mov ax,ax` (`ASM.ASM:65`) leaves a stale `bx`, which never reaches `bp`
//!   ([`project_xz`]);
//! - an off-screen shadow skips the gravity integration and overwrites the
//!   stored shadow offset with a background word ([`drawdots`]);
//! - the shadow colour 87 lies inside the floor's grey ramp (64-163);
//! - `j` is never initialized (`MAIN.C:87`); [`INITIAL_J`] is fitted (A19).

use sr_engine::part::{Part, PartCtx, PartStatus};
use sr_engine::Palette;

use crate::parts::mode_view;
use crate::parts::msrand::MsRand;
use crate::parts::sin1024::SIN1024;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "minvball";

// ------------------------------------------------------------- the screen --

/// `MAIN.C:125` `mov ax,13h`: 320×200, one byte a pixel, rows packed.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
/// `bx` is a 16-bit word, so the drawing addresses the whole 64 KB segment;
/// `ASM.ASM:177`'s `bx+641` reaches 64,640.
const FRAME_MEMORY: usize = 65_536;

// -------------------------------------------------------------- the dots --

/// `MAIN.C:91` `dotnum=512`.
const DOTNUM: usize = 512;
/// `MAIN.C:84` `int rota=-1*64`; `rot` starts at 0 (`:86`).
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

/// The initial `j`. `MAIN.C:87` declares it and `:200` reads it unassigned;
/// the reference's overlaps select 88 (`docs/fidelity/minvball.md` §4).
const INITIAL_J: usize = 88;

// -------------------------------------------------------------- the clock --

/// `MAIN.C:185` `frame<2450`: the loop draws frames 1 through 2,450.
const LAST_FRAME: i32 = 2450;
/// `MAIN.C:189` `if(frame>2300) setpalette(pal2)`.
const PALETTE_AFTER: i32 = 2300;
/// `MAIN.C:242-269`: the blue ramp from 2360, every entry to black from 2400,
/// still from 2440.
const BLUE_RAMP_FRAME: i32 = 2360;
const FADE_FRAME: i32 = 2400;
const FADE_END_FRAME: i32 = 2440;
/// `MAIN.C:171-183` `for(b=64;b>=0;b--)` with two `dis_waitb()` calls a step.
const FADE_STEPS: u32 = 65;
/// The tick the main loop's first iteration runs on.
const MAIN_FIRST_TICK: u32 = FADE_STEPS * 2;

// ---------------------------------------------------------------- the cue --

/// `MAIN.C:191-195`: `if(a>-4 && a<0) break` with `a = dis_musplus()`.
const CUE_LOW: i32 = -4;
const CUE_HIGH: i32 = 0;

/// Polls `init` makes on a fresh start. The shell seeks `MUSIC1` to order 75;
/// the reference publishes 75:61 at the part's tick 0
/// (`reference/music/second3b/trace.csv`, boot 10,368), the seeked player's
/// 257th poll, so 256 pre-polls leave the engine's tick-0 poll as that one.
const MUSIC_PRE_ROLL: u32 = 256;

// ------------------------------------------------------------- the tables --

/// `MAIN.C:32-40`: `isin(deg) = sin1024[deg&1023]`.
fn isin(deg: i32) -> i32 {
    i32::from(SIN1024[(deg & 1023) as usize])
}

/// `MAIN.C:37-40`: `icos(deg) = sin1024[(deg+256)&1023]`.
fn icos(deg: i32) -> i32 {
    i32::from(SIN1024[((deg + 256) & 1023) as usize])
}

/// `MAIN.C:16-19` `depthtable1/2/3`, 128 dwords each.
type DepthTables = [[u32; DEPTH_ENTRIES]; 3];

// --------------------------------------------------------------- the dot --

/// One `struct dot` (`MAIN.C:42-52`, `ASM.ASM:36`): `old1` is the stored
/// shadow offset and `old2` the stored ball offset.
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
    /// `MAIN.C:77` `dottaul[1024]`; the port keeps the 512 used entries.
    order: Vec<u16>,
    /// `MAIN.C:200`'s `j`, the read cursor into `order`.
    next: usize,
    /// `MAIN.C:83` `frame`.
    frame: i32,
    /// `MAIN.C:89` `f`, the stage animation phase.
    f: i32,
    dropper: i16,
    /// `grav`, which `:281` copies to the assembly's `gravity` each frame.
    grav: i16,
    /// `MAIN.C:84-86`.
    rota: i32,
    rot: i32,
    rots: i32,
    rotcos: i32,
    rotsin: i32,
    /// `MAIN.C:27-28` `char pal[768], pal2[768]`: six-bit channels.
    pal: [u8; 768],
    pal2: [u8; 768],
    depth: DepthTables,
    /// `MAIN.C:13` `bgpic`; the full segment, so the buggy restore of a
    /// background word as an offset reads defined (zero) bytes past 64,000.
    bgpic: Vec<u8>,
    /// `MAIN.C:25` `vram`, the 64 KB VGA segment.
    video: Vec<u8>,
    rng: MsRand,
    /// The fade's tick counter, 0 through 129.
    fade_tick: u32,
    phase: Phase,
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
            rota: ROTA_START,
            rot: 0,
            rots: 0,
            rotcos: 0,
            rotsin: 0,
            pal: [0; 768],
            pal2: [0; 768],
            depth: [[0; DEPTH_ENTRIES]; 3],
            bgpic: vec![0; FRAME_MEMORY],
            video: vec![0; FRAME_MEMORY],
            rng: MsRand::new(),
            fade_tick: 0,
            phase: Phase::Fade,
        }
    }

    /// `MAIN.C:92-123`: the `dottaul` shuffle, the initial positions and the
    /// scramble. Every dot starts at `(0, 2560-dropper, 0)`, so the scramble
    /// moves nothing, but its 1,000 `rand()` calls set the scatter's stream.
    fn setup_dots(rng: &mut MsRand) -> (Vec<u16>, Vec<Dot>) {
        let mut order: Vec<u16> = (0..DOTNUM as u16).collect();
        for _ in 0..500 {
            let b = (rng.next() % DOTNUM as i32) as usize;
            let c = (rng.next() % DOTNUM as i32) as usize;
            order.swap(b, c);
        }
        let start = Dot { y: 2560 - DROPPER_START, ..Dot::default() };
        let mut dots = vec![start; DOTNUM];
        for _ in 0..500 {
            let b = (rng.next() % DOTNUM as i32) as usize;
            let c = (rng.next() % DOTNUM as i32) as usize;
            let (p, q) = (dots[b], dots[c]);
            (dots[b].x, dots[b].y, dots[b].z) = (q.x, q.y, q.z);
            (dots[c].x, dots[c].y, dots[c].z) = (p.x, p.y, p.z);
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
        pal[255 * 3..255 * 3 + 3].copy_from_slice(&[31, 0, 15]);
        // `:139-147` the 100 greys at entries 64-163.
        for a in 0..100u32 {
            let c = 64 - 256 / (a + 4);
            let c = c * c / 64;
            let v = (c / 4) as u8;
            pal[(64 + a as usize) * 3..(65 + a as usize) * 3].fill(v);
        }
        pal
    }

    /// `MAIN.C:156-167`: one depth table's value for entry `a`.
    fn depth_value(a: usize, base: u32) -> u32 {
        // `c=a-(43+20)/2; c=c*3/4; c+=8;` clamp to 0..15; `c=15-c`.
        let c = (a as i32 - (43 + 20) / 2) * 3 / 4;
        let c = 15 - (c + 8).clamp(0, 15);
        base.wrapping_add(0x0404_0404u32.wrapping_mul(c as u32))
    }

    /// The three tables `MAIN.C:163-166` builds (`depthtable4` is commented out).
    fn build_depth_tables() -> DepthTables {
        [0x0202, 0x0203_0302, 0x0202]
            .map(|base| std::array::from_fn(|a| Self::depth_value(a, base)))
    }

    /// One iteration of `MAIN.C:196-282`'s `while(repeat--)`.
    fn physics(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        if frame == 500 {
            self.f = 0;
        }
        let i = self.order[self.next] as usize;
        self.next = (self.next + 1) % DOTNUM;
        let dot = &mut self.dots[i];
        if frame < 500 {
            // `:202-208`.
            dot.x = (isin(self.f * 11) * 40) as i16;
            dot.y = (icos(self.f * 13) * 10 - i32::from(self.dropper)) as i16;
            dot.z = (isin(self.f * 17) * 40) as i16;
            dot.yadd = 0;
        } else if frame < 900 {
            // `:209-215`.
            dot.x = (icos(self.f * 15) * 55) as i16;
            dot.y = self.dropper;
            dot.z = (isin(self.f * 15) * 55) as i16;
            dot.yadd = -260;
        } else if frame < 1700 {
            // `:216-223`.
            let a = isin(frame & 1023) / 8;
            dot.x = (icos(self.f * 66) * a) as i16;
            dot.y = 8000;
            dot.z = (isin(self.f * 66) * a) as i16;
            dot.yadd = -300;
        } else if frame < BLUE_RAMP_FRAME {
            // `:224-241`.
            dot.x = (self.rng.next() - 16384) as i16;
            dot.y = (8000 - self.rng.next() / 2) as i16;
            dot.z = (self.rng.next() - 16384) as i16;
            dot.yadd = 0;
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
            self.pal2.fill((63 - a * 2).max(0) as u8);
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
    }

    /// One fade tick: on odd ticks, one upload.
    fn fade(&mut self, ctx: &mut PartCtx) {
        let t = self.fade_tick;
        if t % 2 == 1 {
            let b = (FADE_STEPS - 1 - (t - 1) / 2) as i32;
            for c in 0..768 {
                self.pal2[c] = (i32::from(self.pal[c]) - b).max(0) as u8;
            }
            ctx.set_palette(Palette::from_six_bit(&self.pal2));
        }
        self.fade_tick += 1;
        if self.fade_tick == MAIN_FIRST_TICK {
            self.phase = Phase::Main;
        }
    }

    /// `MAIN.C:185-284`'s loop body for one tick.
    fn main_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `while(!dis_exit() && frame<2450)`.
        if ctx.exit_requested || self.frame >= LAST_FRAME {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        // `:189`.
        if self.frame > PALETTE_AFTER {
            ctx.set_palette(Palette::from_six_bit(&self.pal2));
        }
        // `:191-195`, always in demo.
        let reading = ctx.musplus();
        if reading > CUE_LOW && reading < CUE_HIGH {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        self.physics();
        self.draw_video();
        ctx.surface.pixels_mut()[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
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
            self.grav,
            &self.depth,
        );
    }

    /// The deterministic state of `MAIN.C:92-169`, from the live DAC (whose
    /// entries 164-254 the read-back keeps). The j-fit test builds it without a
    /// `PartCtx`.
    fn setup_state(&mut self, live: &Palette) {
        *self = Minvball::new();
        // `:92-123`.
        let (order, dots) = Self::setup_dots(&mut self.rng);
        self.order = order;
        self.dots = dots;

        // `:127-150`: build the palette, read it back.
        let mut pal = [0u8; 768];
        for entry in 0..256usize {
            pal[entry * 3..entry * 3 + 3].copy_from_slice(&live.entry(entry as u8));
        }
        let built = Self::build_palette();
        pal[..164 * 3].copy_from_slice(&built[..164 * 3]);
        pal[255 * 3..].copy_from_slice(&built[255 * 3..]);
        self.pal = pal;
        self.pal2 = pal;

        // `:152-155`: the mode set's clear and rows 100-199's ramp; then
        // `:168-169` `memcpy(bgpic,vram,64000)`.
        for a in 0..100usize {
            let row = 100 + a;
            self.video[row * 320..(row + 1) * 320].fill((a + 64) as u8);
        }
        self.bgpic[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
        // `:156-167`.
        self.depth = Self::build_depth_tables();
    }
}

impl Part for Minvball {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `MAIN.C:79-169`, everything before the fade's first `dis_waitb`.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), sr_engine::part::PartError> {
        // The head start belongs to the entry run alone: a part reached
        // mid-chain is handed a player already through this stretch.
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
        // `MAIN.C:125-126`: mode 13h, its picture up before the first wait.
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(SCREEN_WIDTH), Some(0));
        ctx.surface.pixels_mut()[..VIDEO_BYTES].copy_from_slice(&self.video[..VIDEO_BYTES]);
        Ok(())
    }

    /// One tick of `MAIN.C:171-284`.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Fade if ctx.exit_requested => {
                self.phase = Phase::Done;
                PartStatus::Finished
            }
            Phase::Fade => {
                self.fade(ctx);
                PartStatus::Continuing
            }
            Phase::Main => self.main_tick(ctx),
        }
    }
}

// ------------------------------------------------------------ the drawing --

/// `ASM.ASM:61-105`: the x/z projection. Returns `(x, shadow row, bp)`.
///
/// `X*sin`'s high word goes to `cx`, then `sub ax,bx` / `sub dx,cx`
/// (`:69-70`): `sub dx,cx` ignores the carry and `ax` is overwritten at `:74`,
/// so the low word, the stale `bx` and `mov ax,ax` (`:65`) never reach `bp`,
/// which is `hi(Z*cos) - hi(X*sin) + 9000` (`bugs.md` §18.1).
fn project_xz(dot: &Dot, rotcos: i32, rotsin: i32) -> (i16, i16, i16) {
    let xsin = i32::from(dot.x) * rotsin;
    let zcos = i32::from(dot.z) * rotcos;
    let high = ((zcos >> 16) as u16).wrapping_sub((xsin >> 16) as u16);
    let bp = (high as i16).wrapping_add(9000);

    // `:74-93`: x' = X*cos + Z*sin; the screen x is (x'>>8 + x'>>11)/bp+160.
    let x32 = i32::from(dot.x)
        .wrapping_mul(rotcos)
        .wrapping_add(i32::from(dot.z).wrapping_mul(rotsin));
    let v = (x32 >> 8).wrapping_add(x32 >> 11);
    let x = ((v / i32::from(bp)) as i16).wrapping_add(160);

    // `:98-105`: the shadow's row, (8<<16)/bp + 100.
    let shadow = (((8 << 16) / i32::from(bp)) as i16).wrapping_add(100);
    (x, shadow, bp)
}

/// `ASM.ASM:120-141`: the ball row after the gravity integration,
/// `(y<<6)/bp + 100`.
fn project_y(y: i16, bp: i16) -> i16 {
    (((i32::from(y) << 6) / i32::from(bp)) as i16).wrapping_add(100)
}

/// `ASM.ASM:122-136`: the y integration and its floor bounce.
fn integrate(dot: &mut Dot, gravity: i16) {
    dot.yadd = dot.yadd.wrapping_add(gravity);
    dot.y = dot.y.wrapping_add(dot.yadd);
    if dot.y >= GRAVITY_BOTTOM {
        // `neg ax; imul gravityd; sar ax,4`: the product's low word.
        dot.yadd = dot.yadd.wrapping_neg().wrapping_mul(GRAVD) >> 4;
        dot.y = dot.y.wrapping_add(dot.yadd);
    }
}

/// Restore `bytes` bytes of `bgpic` at `at` into the video segment.
fn restore(video: &mut [u8], bgpic: &[u8], at: u16, bytes: usize) {
    let at = at as usize;
    video[at..at + bytes].copy_from_slice(&bgpic[at..at + bytes]);
}

/// Restore the old ball's three rows (`ASM.ASM:150-161`, `:185-193`).
fn restore_ball(video: &mut [u8], bgpic: &[u8], old2: u16) {
    restore(video, bgpic, old2, 4);
    restore(video, bgpic, old2.wrapping_add(320), 4);
    restore(video, bgpic, old2.wrapping_add(640), 4);
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
/// The dots are drawn in array order with no depth test (`bugs.md` §2). The
/// gravity integration sits in the ball block, which `ja @@2` (`:96`, `:105`)
/// skips when the shadow is off screen (§18.3). There, `:194-196`'s `mov
/// ds:[si+6],ax` stores as the shadow offset the low word of the last dword
/// restored, `bgpic[old2+640..644]` (§18.2); the next frame restores at that
/// word, so writes reach 64,642 and `bgpic` is zero past 64,000 (D24).
fn drawdots(
    dots: &mut [Dot],
    bgpic: &[u8],
    video: &mut [u8],
    rotcos: i32,
    rotsin: i32,
    gravity: i16,
    depth: &DepthTables,
) {
    for dot in dots.iter_mut() {
        let (x, shadow, bp) = project_xz(dot, rotcos, rotsin);
        if (x as u16) <= 319 && (shadow as u16) <= 199 {
            // `:106-118`: restore the old shadow and write the two-pixel one.
            let at = (shadow as u16).wrapping_mul(320).wrapping_add(x as u16);
            restore(video, bgpic, dot.old1, 2);
            video[at as usize..at as usize + 2].fill(SHADOW);
            dot.old1 = at;

            // `:120-141`: the y integration and the ball row.
            integrate(dot, gravity);
            let row = project_y(dot.y, bp);
            restore_ball(video, bgpic, dot.old2);
            if (x as u16) <= 319 && (row as u16) <= 199 {
                // `:162-177`: the depth entry and the three rows.
                let at = (row as u16).wrapping_mul(320).wrapping_add(x as u16);
                let entry = (((bp as u16) >> 6) & 0xFFFC) as usize / 4;
                write_table(video, at.wrapping_add(1), depth[0][entry], 2);
                write_table(video, at.wrapping_add(320), depth[1][entry], 4);
                write_table(video, at.wrapping_add(641), depth[2][entry], 2);
                dot.old2 = at;
            }
            // Otherwise `@@3` (`:203-216`): the ball is off screen and keeps
            // `old2`.
        } else {
            // `@@2` (`:185-202`).
            let third = dot.old2.wrapping_add(640) as usize;
            let stored = u16::from_le_bytes([bgpic[third], bgpic[third + 1]]);
            restore_ball(video, bgpic, dot.old2);
            restore(video, bgpic, dot.old1, 2);
            dot.old1 = stored;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{differing_pixels, reference, Reading, Rig, ScriptedMusic};
    use sr_engine::dac_expand;
    use sr_engine::music::Music;

    /// A player scripted one `musplus` reading a tick; a tick past the
    /// script reads 0.
    fn scripted(script: Vec<i32>) -> ScriptedMusic {
        ScriptedMusic::musplus(script)
            .past_end(Reading { musplus: 0, ..Reading::default() })
            .never_syncs("`DOTS/MAIN.C` never syncs: its music call is `dis_musplus`")
    }

    fn quiet(last_tick: u32) -> ScriptedMusic {
        scripted(vec![0; last_tick as usize + 1])
    }

    fn rig(music: ScriptedMusic) -> Rig {
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(music)
    }

    fn started_with(music: ScriptedMusic) -> (Rig, Minvball) {
        let mut rig = rig(music);
        let mut part = Minvball::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Minvball) {
        started_with(quiet(0))
    }

    /// Step the part until the rig's tick is `tick`, asserting it keeps going.
    fn run_to(rig: &mut Rig, part: &mut Minvball, tick: u32) {
        while rig.tick < tick {
            let at = rig.tick;
            assert_eq!(rig.step(part), PartStatus::Continuing, "tick {at}");
        }
    }

    /// The one-time fade, so a test can reach the main loop.
    fn run_fade(rig: &mut Rig, part: &mut Minvball) {
        run_to(rig, part, MAIN_FIRST_TICK);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(part.frame, 0);
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
        for dot in &dots {
            assert_eq!((dot.x, dot.y, dot.z, dot.yadd), (0, -19440, 0, 0));
            assert_eq!((dot.old1, dot.old2), (0, 0));
        }
        // The main loop's first event reads `dottaul[88]` (`INITIAL_J`).
        assert_eq!(order[INITIAL_J], 97);
    }

    // --- the cue and the exit ------------------------------------------------

    #[test]
    fn the_cue_window_is_the_open_interval_minus_four_to_zero() {
        // `MAIN.C:194` `if(a>-4 && a<0) break`: -4 and 0 are *outside*.
        for (reading, fires) in [(-5, false), (-4, false), (-3, true), (-2, true), (-1, true), (0, false), (1, false)] {
            let (mut rig, mut part) = started_with(quiet(1_000));
            run_fade(&mut rig, &mut part);
            let tick = rig.tick as usize;
            rig.player().script[tick].musplus = reading;
            let status = rig.step(&mut part);
            if fires {
                assert_eq!(status, PartStatus::Finished, "musplus {reading} must break");
                assert_eq!(part.phase, Phase::Done, "musplus {reading}");
            } else {
                assert_eq!(status, PartStatus::Continuing, "musplus {reading} is outside the window");
                assert_eq!(part.phase, Phase::Main, "musplus {reading}");
            }
        }
    }

    #[test]
    fn a_skipped_cue_leaves_the_part_continuing_and_drawing() {
        // The window's two neighbours and a jump across it.
        let mut script = vec![0i32; 1_000];
        for (tick, reading) in [(140usize, -5), (150, -4), (160, 0), (170, 1)] {
            script[tick] = reading;
        }
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, 300);
        assert_eq!(part.phase, Phase::Main);
        assert_ne!(
            &rig.surface.pixels()[..VIDEO_BYTES],
            &part.bgpic[..VIDEO_BYTES],
            "it is drawing"
        );
    }

    #[test]
    fn exit_requested_finishes_on_that_tick_in_both_phases() {
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 20);
        assert_eq!(part.phase, Phase::Fade);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);

        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
    }

    #[test]
    fn the_frame_cap_ends_the_loop_at_two_thousand_four_hundred_fifty() {
        let (mut rig, mut part) = started();
        run_fade(&mut rig, &mut part);
        // Fast-forward the physics without drawing every frame.
        while part.frame < LAST_FRAME {
            part.physics();
        }
        assert_eq!(rig.step(&mut part), PartStatus::Finished);
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
        assert_eq!(part.f, 499);
        part.physics();
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
        let [d1, d2, d3] = Minvball::build_depth_tables();
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
        assert_eq!(&part.pal[..164 * 3], &built[..164 * 3]);
        assert_eq!(&part.pal[255 * 3..255 * 3 + 3], &[31, 0, 15]);
        // `:151` zeroes the DAC after the read-back.
        assert!((0..=255u8).all(|i| rig.palette.entry(i) == [0, 0, 0]));
        // The floor's ramp: row 150 carries index 114.
        assert_eq!(part.video[150 * 320], 114);
        assert_eq!(part.bgpic[150 * 320], 114);
        assert!(part.video[..100 * 320].iter().all(|p| *p == 0));
        // The shadow's entry 87 is a grey of the floor's ramp: 11, widened to 45.
        assert_eq!(&built[87 * 3..88 * 3], &[11, 11, 11]);
        assert_eq!(dac_expand(built[87 * 3]), 45);
    }

    // --- the drawing ---------------------------------------------------------

    #[test]
    fn the_shadow_restore_bug_stores_the_old_balls_third_row_word() {
        // At rot = 128 (45°) this dot projects to x = 345, off screen.
        let dot = Dot { x: 16384, z: 16384, ..Dot::default() };
        let (x, shadow, bp) = project_xz(&dot, 11584, 11584);
        assert_eq!((x, shadow, bp), (345, 158, 9000));
        let mut dots = [dot];
        let mut bgpic = vec![0u8; FRAME_MEMORY];
        // The old ball's third row (old2 = 0, so bgpic[640..]) carries a
        // marker word; a second marker at the shadow's would-be row.
        bgpic[640..644].copy_from_slice(&[0x34, 0x12, 0x99, 0x88]);
        bgpic[158..160].copy_from_slice(&[0xAA, 0xBB]);
        let mut video = vec![0u8; FRAME_MEMORY];
        let depth = [[0u32; DEPTH_ENTRIES]; 3];
        drawdots(&mut dots, &bgpic, &mut video, 11584, 11584, 3, &depth);
        assert_eq!(
            dots[0].old1, 0x1234,
            "the low word of bgpic[old2+640..644], not the shadow row"
        );
        assert_eq!(dots[0].old2, 0, "the off-screen ball keeps old2");
        // The next frame restores at that word as a pixel offset.
        bgpic[0x1234..0x1236].copy_from_slice(&[0xCC, 0xDD]);
        dots[0].x = 0;
        dots[0].z = 0;
        drawdots(&mut dots, &bgpic, &mut video, 16384, 0, 3, &depth);
        assert_eq!(&video[0x1234..0x1236], &[0xCC, 0xDD]);
    }

    /// `bugs.md` §2: no depth test, so the draw order alone decides whether a
    /// later, farther dot's shadow covers an earlier, nearer dot's ball.
    #[test]
    fn shadows_are_drawn_over_balls_without_a_depth_test() {
        // Rotation 0: cos 16384, sin 0, so x' = X and bp = Z/4 + 9000.
        let (cos, sin) = (16384, 0);
        // The near dot (bp 8000): its ball's wide middle row is row 158,
        // columns 160..=163.
        let near = Dot { y: 7125, z: -4000, ..Dot::default() };
        // The far dot (bp 9000): its shadow is row 158, columns 160 and 161.
        let far = Dot::default();
        let (near_x, near_shadow, near_bp) = project_xz(&near, cos, sin);
        assert_eq!((near_x, near_shadow, near_bp), (160, 165, 8000));
        assert_eq!(project_y(near.y, near_bp), 157);
        let (far_x, far_shadow, _) = project_xz(&far, cos, sin);
        assert_eq!((far_x, far_shadow), (160, 158));

        let depth = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        let paint = |mut dots: [Dot; 2]| -> Vec<u8> {
            let mut video = vec![0u8; FRAME_MEMORY];
            drawdots(&mut dots, &bgpic, &mut video, cos, sin, 0, &depth);
            video
        };
        let middle = 158 * 320 + 160;
        let ball = depth[1][(((near_bp as u16) >> 6) & 0xFFFC) as usize / 4].to_le_bytes();
        assert!(!ball.contains(&SHADOW));

        let video = paint([near, far]);
        assert_eq!(
            &video[middle..middle + 4],
            &[SHADOW, SHADOW, ball[2], ball[3]],
            "the later, farther shadow covers the nearer ball"
        );
        let video = paint([far, near]);
        assert_eq!(&video[middle..middle + 4], &ball, "the later, nearer ball covers the shadow");
    }

    /// `bugs.md` §18.1: `bp = hi(Z*cos) - hi(X*sin) + 9000`, whatever the
    /// previous dot left in `bx`.
    #[test]
    fn the_stale_bx_never_reaches_bp() {
        // Z*cos = -807 * 65536 has low word 0: a borrow from a larger `bx`
        // would move bp by one and the shadow row from 163 to 164.
        let dot = Dot { z: -3228, ..Dot::default() };
        assert_eq!(project_xz(&dot, 16384, 0), (160, 163, 8193));

        // Drawn after another dot, it casts the same shadow as drawn alone.
        let depth = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        let mut alone = vec![0u8; FRAME_MEMORY];
        drawdots(&mut [dot], &bgpic, &mut alone, 16384, 0, 0, &depth);
        let mut after = vec![0u8; FRAME_MEMORY];
        drawdots(&mut [Dot::default(), dot], &bgpic, &mut after, 16384, 0, 0, &depth);
        assert_eq!(after[163 * 320 + 160], SHADOW);
        assert_eq!(alone[163 * 320 + 160], SHADOW);
        assert_eq!(after[164 * 320 + 160], 0);
    }

    /// `bugs.md` §18.3: an off-screen shadow skips the gravity integration.
    #[test]
    fn an_off_screen_shadow_holds_the_dots_y_still() {
        let depth = Minvball::build_depth_tables();
        let bgpic = vec![0u8; FRAME_MEMORY];
        let mut video = vec![0u8; FRAME_MEMORY];
        // At 45 degrees this dot's x projects to 345, off screen.
        let mut dots = [Dot { x: 16384, y: 100, z: 16384, yadd: 7, ..Dot::default() }];
        drawdots(&mut dots, &bgpic, &mut video, 11584, 11584, 3, &depth);
        assert_eq!((dots[0].y, dots[0].yadd), (100, 7), "gravity is skipped");
        // On screen it integrates: yadd += gravity, then y += yadd.
        let mut dots = [Dot { y: 100, yadd: 7, ..Dot::default() }];
        drawdots(&mut dots, &bgpic, &mut video, 16384, 0, 3, &depth);
        assert_eq!((dots[0].yadd, dots[0].y), (10, 110), "gravity runs on screen");
    }

    #[test]
    fn the_ball_rows_are_the_three_depth_table_writes() {
        // A dot at the origin: x = 160, ball row 100, bp 9000; the three rows
        // are the tables' bytes at bx+1, bx+320 and bx+641.
        let mut dots = [Dot::default()];
        let bgpic = vec![0u8; FRAME_MEMORY];
        let mut video = vec![0u8; FRAME_MEMORY];
        let depth = Minvball::build_depth_tables();
        drawdots(&mut dots, &bgpic, &mut video, 16384, 0, 0, &depth);
        let at = 32_160usize;
        assert_eq!(dots[0].old2 as usize, at);
        let entry = ((9000u16 >> 6) & 0xFFFC) as usize / 4;
        assert_eq!(&video[at + 1..at + 3], &(depth[0][entry] as u16).to_le_bytes());
        assert_eq!(&video[at + 320..at + 324], &depth[1][entry].to_le_bytes());
        assert_eq!(&video[at + 641..at + 643], &(depth[2][entry] as u16).to_le_bytes());
    }

    #[test]
    fn the_music_pre_roll_puts_the_reference_state_at_tick_zero() {
        // The trace at boot 10,368 (tick 0) reads 75:61, `zplus` 1, `musplus` -3.
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(75);
        let mut music = PlayerMusic::new(player.clone(), 0);
        // The pre-roll, then the engine's tick-0 poll.
        for _ in 0..=MUSIC_PRE_ROLL {
            music.poll();
        }
        let position = music.position();
        assert_eq!((position.order, position.row), (75, 61));
        assert_eq!(lock(&player).zplus(), 1);
        assert_eq!(music.musplus(), -3);
        // After the fade the main loop's first tick reads order 77, the
        // trace's position at boot 10,498.
        for _ in 0..MAIN_FIRST_TICK {
            music.poll();
        }
        let position = music.position();
        assert_eq!(position.order, 77);
        assert!((28..=30).contains(&position.row), "row {}", position.row);
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        let mut rig = rig(quiet(0));
        let mut part = Minvball::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player().polls, 0, "a mid-chain start polls nothing");
    }

    #[test]
    fn the_fade_uploads_on_its_odd_ticks_and_ends_with_the_full_palette() {
        // Tick 0: no upload. Tick 1: `pal - 64`, black. Tick 129: `pal`.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 2);
        assert!((0..=255u8).all(|i| rig.palette.entry(i) == [0, 0, 0]));
        run_to(&mut rig, &mut part, MAIN_FIRST_TICK);
        for entry in 0..=163usize {
            assert_eq!(
                rig.palette.entry(entry as u8)[..],
                part.pal[entry * 3..entry * 3 + 3],
                "entry {entry} at the last upload"
            );
        }
    }

    // --- the uninitialized `j` -----------------------------------------------

    /// The main loop replayed from frame 1 to `frame` with the cursor at `j`,
    /// as RGB through `pal` (the frames are before the 2,300 palette uploads).
    fn replay(j: usize, frame: i32) -> Vec<u8> {
        let mut part = Minvball::new();
        part.setup_state(&Palette::new());
        part.next = j;
        for _ in 1..=frame {
            part.physics();
            part.draw_video();
        }
        part.video[..VIDEO_BYTES]
            .iter()
            .flat_map(|&byte| {
                let at = byte as usize * 3;
                part.pal[at..at + 3].iter().map(|&c| dac_expand(c)).collect::<Vec<_>>()
            })
            .collect()
    }

    /// The fit `docs/fidelity/minvball.md` §4 and A19 name: the reference's
    /// 10,900 frame (frame counter 403) under each of the 512 cursors.
    #[test]
    #[ignore = "measurement: the 512-value j fit; run with --release --ignored --nocapture"]
    fn the_initial_j_is_fitted_to_the_reference() {
        let reference = reference("minvball", 10_900, "rgb");
        let mut scores: Vec<(usize, usize)> =
            (0..DOTNUM).map(|j| (differing_pixels(&replay(j, 403), &reference), j)).collect();
        scores.sort_unstable();
        println!("best five (pixels, j): {:?}", &scores[..5]);
        assert_eq!(scores[0].1, INITIAL_J, "the fitted cursor is the minimum over 0..512");
        assert_eq!(
            (scores[0].0, scores[1].0),
            (0, 283),
            "88 is the only cursor with no residue; the runner-up's pixels"
        );
    }
}
