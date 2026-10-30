//! COMAN — the 3D sinus field (`3dsinfld`).
//!
//! `COMAN/MAIN.C` (228 lines) over `COMAN/ASM.ASM` (`inittwk`, `docol`,
//! `docopy`, `setpalarea`) and `COMAN/THELOOP.INC`, the 5,230-line unrolled
//! ray march `COMAN/DOLOOP.C` generates, with the linked wave tables
//! `W1DTA.BIN`/`W2DTA.BIN` (`CALCW12` is off, `MAIN.C:6`). The acts:
//!
//! 1. `MAIN.C:118-227`: [`palette`], the 64 KB vbuf and the cleared screen.
//! 2. `MAIN.C:45-116` `doit`: the gate `musplus < 0`, then while
//!    `frame < 4444` the break `-8 < a < 0`, the `frepeat` bodies moving
//!    `startrise`, the sine camera, 160 [`docol`] columns, the two-page flip
//!    (CRTC `0x0C` = `0x80`/`0x00`) and [`docopy`].
//! 3. `THELOOP.INC`: the swapped 16.16 march, unit for unit in [`theloop`].
//!
//! `inittwk` (`ASM.ASM:36-78`) turns chain-4 off and leaves the row at 80
//! plane bytes; the emulator's unchained handler stores a CPU address as four
//! plane bytes that the display reads back as four pixels, so `docopy`'s
//! plane-pair writes show each of the vbuf's 160 columns twice ([`compose`]).
//!
//! `dis_waitb` returns the copper frames elapsed; one render costs two
//! (`reference-indices.md` §16.3), so an iteration runs every second tick, and
//! the frame-4 iteration's two extra waits make the ticks `0, 2, 4, 8, 10, …`
//! ([`Coman::iteration`]). A picture shows from the next iteration's tick, for
//! two ticks. The reference's per-keyframe origins drift (A26), so the mapping
//! is stated per keyframe (`docs/fidelity/3dsinfld.md` §6), and `init` makes
//! the music's head start ([`MUSIC_PRE_ROLL`]). The part ends on the music
//! break in order 96's tail, at tick 2,014.
//!
//! Preserved bugs (`bugs.md` §20): every `combg` index overruns its 16 bytes
//! and the `PXLSUX` background is not assembled ([`combg_overrun`]); the
//! palette passes overwrite each other ([`palette`]); `docol` masks all four
//! coordinates; `docopy`'s clear moves the copy to row 70; `cameralevel`
//! reaches only `theloop_heigth`, which nothing reads; the frame-4
//! `setpalarea` re-uploads the same palette and the 4,444-frame cap is never
//! reached.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::sin1024::SIN1024;
use crate::parts::{asset, mode_view, read_asset};

/// The part's registry name: what `--part` accepts.
const NAME: &str = "3dsinfld";

// ---------------------------------------------------------------- assets --

/// The linked wave tables (`MAIN.C:212-213` `wave1 = w1dta`, `wave2 = w2dta`).
const WAVE_PATHS: [&str; 2] = ["assets/COMAN/W1DTA.BIN", "assets/COMAN/W2DTA.BIN"];
/// A wave table is 32,768 signed words; `fs:[si]`/`gs:[di]` wrap at 64 KB.
const WAVE_BYTES: usize = 65_536;
/// The two wave tables, X and Y.
type Waves = [Vec<u8>; 2];

// --------------------------------------------------------------- screens --

const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// `halloc(16384,4)`: the 64 KB vbuf, addressed through a 16-bit `bp`.
const VBUF_BYTES: usize = 65_536;
/// The unchained VGA's 64 K plane addresses, four bytes each
/// (`VGA_UnchainedVGA_Handler`'s `((uint32_t*)linear)[addr]`): 256 KB.
const VIDEO_BYTES: usize = 4 * 65_536;
/// The flip's two pages: segment `a000` and `a800` (plane address `0x8000`).
const PAGE_LO: u16 = 0x0000;
const PAGE_HI: u16 = 0x8000;
/// `docopy`: 18 rows cleared at destination row 52, then 140 rows from vbuf
/// row 60 to destination row 70 + `startrise`.
const COPY_ROWS: usize = 140;
const COPY_SOURCE_ROW: usize = 60;
const CLEAR_DEST_ROW: usize = 52;
const CLEAR_DEST_ROWS: usize = 18;
const COPY_DEST_ROW: usize = CLEAR_DEST_ROW + CLEAR_DEST_ROWS;
/// `docopy`'s post-copy clear: vbuf rows 68..97.
const CLEAR_FIRST_ROW: usize = 68;
const CLEAR_ROWS: usize = 30;

// ---------------------------------------------------------------- tables --

/// `MAIN.C:85-98`: the field of view.
const COLUMNS: i32 = 160;
/// `MAIN.C:48`: the rise counter's initial and maximum value.
const STARTRISE_MAX: i32 = 160;
/// `MAIN.C:57`: the frame cap.
const FRAME_CAP: u32 = 4444;
/// `MAIN.C:59-60`: the music break is `a > -8 && a < 0`.
const BREAK_LOW: i32 = -8;
/// `MAIN.C:63`: `startrise` rises while `a > -30 && a < 0`.
const RISE_LOW: i32 = -30;

/// One render iteration's vblanks, the measured two-frame step.
const VBLANKS_PER_ITERATION: u32 = 2;

/// Polls `init` makes on a fresh start: from `_zplaysong(91)`'s row 0, the
/// 527th leaves the published row at 63 (`musplus` -1), and the engine's
/// tick-0 poll is order 93 row 0, where the gate opens.
const MUSIC_PRE_ROLL: u32 = 527;

/// `DOLOOP.C`: `bail = 192` ray steps; at `bailhalve = 64` the step doubles,
/// so the generated labels are `0..63` and then every second index to 190.
const BAIL: usize = 192;
const BAIL_HALVE: usize = 64;
/// `firstraydir = -(200-horizony)*2560` with `horizony = 70`, swapped
/// (`THELOOP.INC:42`).
const FIRST_RAY_DIR: u32 = 0xEC00_FFFA;

/// `DOLOOP.C`'s `wflip`: the 16-bit halves trade places. The ray state keeps
/// the integer in the low word, so the generated `adc ax,0` / `adc ax,-1`
/// carries bit 31 back into it.
fn wflip(l: u32) -> u32 {
    ((l & 0xFFFF) << 16).wrapping_add(l >> 16)
}

/// `THELOOP.INC`'s `zwave[j] = 16*sin(2*pi*3*j/192)`, truncated by the
/// generator's `int` assignment.
fn zwave(j: usize) -> i16 {
    (16.0 * (j as f64 * std::f64::consts::PI * 2.0 * 3.0 / BAIL as f64).sin()) as i16
}

/// The generated `_@seekj` labels (`DOLOOP.C:89-160`): `0..64`, then
/// `64, 66, ..., 190`.
fn labels() -> impl Iterator<Item = usize> {
    (0..BAIL_HALVE).chain((BAIL_HALVE..BAIL).step_by(2))
}

/// `MAIN.C:8` `char combg[16]`: the shipped link carries no background
/// (`COMBG.*` are not linked, and `docopy`'s `PXLSUX` half that reads
/// `combguse` is not assembled).
const COMBG: [u8; 16] = [0; 16];

/// `MAIN.C:161-164` `palette[x] = combg[16+x]` for `x = 720..767`: indexes
/// 736..783 of the 16-byte array. The bytes cannot be observed (the march's
/// colour is a byte shifted right, at most 127), so the port reads past the
/// array as zero.
fn combg_overrun() -> [u8; 48] {
    std::array::from_fn(|i| COMBG.get(16 + 720 + i).copied().unwrap_or(0))
}

/// `MAIN.C:127-164`: the palette, six bits a component. The formula writes
/// entries 223 down to 8, the `*9/6` pass brightens 0..239, the dark-red bar
/// writes 255..232, entry 0 is forced black and the `combg` overrun
/// overwrites 240..255 last.
fn palette() -> Palette {
    let mut bytes = [0u8; 768];
    for a in 0..256i32 {
        let uc = ((223 - a * 22 / 26) * 3) as usize;
        let green = (230 - a) / 4 + i32::from(SIN1024[((a * 4) & 1023) as usize]) / 32;
        bytes[uc + 1] = green.clamp(0, 63) as u8;
        bytes[uc + 2] = ((255 - a) / 3).min(63) as u8;
        bytes[uc] = ((40 - (a - 220).abs().min(40)) / 3) as u8;
    }
    for byte in bytes.iter_mut().take(720) {
        *byte = (i32::from(*byte) * 9 / 6).min(63) as u8;
    }
    for a in 0..24i32 {
        let uc = ((255 - a) * 3) as usize;
        bytes[uc..uc + 3].copy_from_slice(&[((a - 4).max(0) / 2) as u8, 0, 0]);
    }
    bytes[..3].fill(0);
    bytes[720..768].copy_from_slice(&combg_overrun());
    Palette::from_six_bit(&bytes)
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
/// column directions, and `bp` the vbuf offset. Labels `j = 0..63` step the
/// wave by `xsina`; `j = 64, 66, ..., 190` by `2*xsina`, adding the ray
/// direction twice. Every write goes through a 16-bit `bp`.
fn theloop(
    vbuf: &mut [u8; VBUF_BYTES],
    waves: &Waves,
    mut si: u16,
    mut di: u16,
    xsina: u16,
    ysina: u16,
    mut bp: u16,
) {
    let mut eax: u32 = 0;
    let mut ecx: u32 = FIRST_RAY_DIR;

    /// `add eax, value; adc ax, addend`: the carry out of bit 31 goes back
    /// into the integer half, which the next `add eax` reads.
    fn add_eax(eax: &mut u32, value: u32, addend: u16) -> u16 {
        let (sum, carry) = add32(*eax, value);
        let ax = adc_ax(sum as u16, addend, carry);
        *eax = (sum & 0xFFFF_0000) | u32::from(ax);
        ax
    }

    for j in labels() {
        let (xs, ys) = if j < BAIL_HALVE {
            (xsina, ysina)
        } else {
            (xsina.wrapping_mul(2), ysina.wrapping_mul(2))
        };
        si = si.wrapping_add(xs);
        let mut bx = wave_word(&waves[0], si) as u16;
        di = di.wrapping_add(ys);
        bx = bx.wrapping_add(wave_word(&waves[1], di) as u16);
        bx = bx.wrapping_add((zwave(j) as i32 - 240) as u16);
        let mut ax = eax as u16;
        if (ax as i16) < (bx as i16) {
            let dl = (bx.wrapping_add((140 - j / 8) as u16) as u8) >> 1;
            let l = wflip((j * 2560) as u32);
            loop {
                ax = add_eax(&mut eax, l, 0);
                vbuf[bp as usize] = dl;
                if (ax as i16) >= (bx as i16) {
                    // `_@seek1a`: the `adc ecx` carries the compare's CF,
                    // which a taken signed `jge` does not zero (`DOLOOP.C:141`
                    // assumes it does).
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
}

/// `docol` (`ASM.ASM:105-132`): all four coordinate arguments masked with ~1
/// (`:115-123`), then the march; `b`, the vbuf offset, is not masked. Its
/// `ax = cameralevel` lands in `theloop_heigth`, which no emitted instruction
/// reads.
fn docol(vbuf: &mut [u8; VBUF_BYTES], waves: &Waves, xw: i32, yw: i32, xa: i32, ya: i32, b: i32) {
    let mask = |v: i32| (v as u16) & 0xFFFE;
    theloop(vbuf, waves, mask(xw), mask(yw), mask(xa), mask(ya), b as u16);
}

/// `docopy` (`ASM.ASM:134-236`), the live half.
///
/// The 18-row `rep stosd` clear at destination row 52 advances `di` over
/// itself (`:178-182`), so the 140 rows of vbuf 60..199 land at row
/// 70 + `startrise` (`5*startrise` paragraphs: one row a unit), each 80-byte
/// half of a vbuf row in one plane pair at the same address. The clear of
/// vbuf rows 68..97 (`:216-229`) comes after the copy, so the next frame
/// sees it.
fn docopy(vbuf: &mut [u8; VBUF_BYTES], video: &mut [u8; VIDEO_BYTES], page: u16, startrise: i32) {
    let base = (u32::from(page).wrapping_add((80 * startrise) as u32)) & 0xFFFF;
    for at in 0..CLEAR_DEST_ROWS * 80 {
        let dest = ((base + (CLEAR_DEST_ROW * 80 + at) as u32) & 0xFFFF) as usize;
        video[4 * dest..4 * dest + 4].fill(0);
    }
    for row in 0..COPY_ROWS {
        let source = (COPY_SOURCE_ROW + row) * 160;
        for k in 0..80 {
            let dest = ((base + (COPY_DEST_ROW + row) as u32 * 80 + k as u32) & 0xFFFF) as usize;
            let (a, b) = (vbuf[source + k], vbuf[source + 80 + k]);
            video[4 * dest..4 * dest + 4].copy_from_slice(&[a, a, b, b]);
        }
    }
    let start = CLEAR_FIRST_ROW * 160;
    vbuf[start..start + CLEAR_ROWS * 160].fill(0);
}

/// The displayed picture from `page`: `M_VGA`'s line reads 320 linear bytes a
/// scanline, and a plane address is four bytes into the unchained memory.
fn compose(video: &[u8; VIDEO_BYTES], page: u16, surface: &mut [u8]) {
    let base = usize::from(page) * 4;
    let rows = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
    surface[..rows].copy_from_slice(&video[base..base + rows]);
}

// ------------------------------------------------------------------ part --

/// The COMAN sinus field.
pub struct Coman {
    waves: Waves,
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
    /// The inner body's count for the next iteration (`while(frepeat--)`).
    frepeat: i32,
    /// `MAIN.C`'s `flip`: 0 or 1, toggled every iteration.
    flip: u8,
    /// The CRTC 0x0C start the last iteration selected.
    start_page: u16,
    /// The part tick the next iteration runs on.
    next_iteration: u32,
    /// The gate has passed.
    started: bool,
    /// The loop has broken or the frame cap is reached.
    done: bool,
}

impl Coman {
    /// A part with nothing loaded: `init` reads the wave tables.
    pub fn new() -> Coman {
        Coman {
            waves: [Vec::new(), Vec::new()],
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
        }
    }

    /// The two wave tables; a short or absent file is a part error.
    fn read_waves() -> Result<Waves, PartError> {
        let read = |path: &str| {
            let bytes = read_asset(NAME, path)?;
            if bytes.len() != WAVE_BYTES {
                return Err(PartError::new(
                    NAME,
                    format!(
                        "{} is {} bytes; the linked table is {WAVE_BYTES}",
                        asset(path),
                        bytes.len()
                    ),
                ));
            }
            Ok(bytes)
        };
        Ok([read(WAVE_PATHS[0])?, read(WAVE_PATHS[1])?])
    }

    /// `MAIN.C:85-98`: one iteration's camera and 160 columns.
    fn render(&mut self) {
        self.rot2 = self.rot2.wrapping_add(4);
        self.rot += i32::from(SIN1024[(self.rot2 & 1023) as usize]) / 15;
        let r = self.rot >> 3;
        let sin = |at: i32| i32::from(SIN1024[(at & 1023) as usize]);
        let (rsin, rcos) = (sin(r), sin(r + 256));
        let (rsin2, rcos2) = (sin(r + 177), sin(r + 177 + 256));
        // `xw`/`yw` are copied before the loop (`MAIN.C:84`): the change at
        // `a == 80` is for the next frame.
        let (xw, yw) = (self.xwav, self.ywav);
        for a in 0..COLUMNS {
            let x = a - 80;
            let y = 160;
            let xa = (x * rcos + y * rsin) / 256;
            let ya = (y * rcos2 - x * rsin2) / 256;
            let b = (a & 1) * 80 + (a >> 1) + 199 * 160;
            docol(&mut self.vbuf, &self.waves, xw, yw, xa, ya, b);
            if a == 80 {
                self.xwav += xa * 4;
                self.ywav += ya * 4;
            }
        }
    }

    /// One main-loop iteration (`MAIN.C:57-114`) with the music reading `a`:
    /// the `frepeat` inner bodies, the render, the flip and the copy.
    fn iteration(&mut self, tick: u32, a: i32) {
        let mut extra_waits = 0u32;
        for _ in 0..self.frepeat {
            if a > RISE_LOW && a < 0 && self.startrise < STARTRISE_MAX {
                self.startrise += 1;
            }
            if self.frame < 400 && self.startrise > 0 {
                if self.frame == 4 {
                    // `MAIN.C:69-73`: `dis_waitb(); setpalarea(...)`; the
                    // upload is the palette in force, the wait a vblank.
                    extra_waits += 1;
                }
                self.startrise -= 1;
            }
        }
        self.render();
        self.flip ^= 1;
        let (start, copy) = if self.flip == 0 { (PAGE_HI, PAGE_LO) } else { (PAGE_LO, PAGE_HI) };
        self.start_page = start;
        self.frame += VBLANKS_PER_ITERATION;
        self.frepeat = VBLANKS_PER_ITERATION as i32;
        docopy(&mut self.vbuf, &mut self.video, copy, self.startrise);
        self.next_iteration = tick + VBLANKS_PER_ITERATION + extra_waits;
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
        self.waves = Coman::read_waves()?;
        ctx.set_palette(palette());
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(SCREEN_WIDTH), Some(0));
        // The mode set and `inittwk`'s clear black the screen before the
        // gate, so a chain does not show the previous part through it.
        compose(&self.video, PAGE_LO, ctx.surface.pixels_mut());
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
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
        if self.frame >= FRAME_CAP || (a > BREAK_LOW && a < 0) {
            self.done = true;
            return PartStatus::Finished;
        }
        self.iteration(ctx.frame, a);
        compose(&self.video, self.start_page, ctx.surface.pixels_mut());
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PlayerMusic;
    use crate::parts::test_rig::{repo, Rig};
    use sr_engine::music::Music;

    #[test]
    fn wflip_swaps_the_halves() {
        assert_eq!(wflip(0xFFFA_EC00), 0xEC00_FFFA);
        assert_eq!(wflip(0), 0);
        assert_eq!(wflip(0x0000_0A00), 0x0A00_0000);
        assert_eq!(wflip(0x0000_0001), 0x0001_0000);
        // `-(200-70)*2560` is `0xFFFAEC00`; the source's `mov ecx,0EC00FFFAh`.
        assert_eq!(wflip((-(130i32 * 2560)) as u32), FIRST_RAY_DIR);
    }

    /// A tiny interpreter for the shipped `THELOOP.INC`, so the march can be
    /// tested against the **source file** rather than against a second
    /// reading of it.
    mod generated {
        use super::super::{wave_word, FIRST_RAY_DIR};
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
        /// hold the masked directions (`ax`, the camera height, is unread).
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
        ) {
            let xsina2 = xsina1.wrapping_mul(2);
            let ysina2 = ysina1.wrapping_mul(2);
            xsina1 &= 0xfffe;
            ysina1 &= 0xfffe;
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
            }
        }
    }

    #[test]
    fn the_ray_march_matches_the_shipped_generated_program() {
        let text = std::fs::read_to_string(repo("upstream/SecondReality/COMAN/THELOOP.INC"))
            .expect("THELOOP.INC");
        let program = generated::parse(&text);
        let waves = Coman::read_waves().unwrap();
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
            docol(&mut mine, &waves, 0, 0, xa, ya, b);
            let mut theirs = Box::new([0u8; VBUF_BYTES]);
            generated::run(
                &program,
                &mut theirs,
                &waves[0],
                &waves[1],
                mask(0),
                mask(0),
                mask(xa),
                mask(ya),
                b as u16,
            );
            assert_eq!(
                &mine[..],
                &theirs[..],
                "column a = {a} is not the shipped program's"
            );
            assert!(mine.iter().any(|&byte| byte != 0), "column a = {a} drew nothing");
        }
    }

    #[test]
    fn the_zwave_is_the_generators_three_sines_at_the_labels_it_emits() {
        let text = std::fs::read_to_string(repo("upstream/SecondReality/COMAN/THELOOP.INC"))
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
        // Entries 240..255 are the `combg[736..784]` overrun, modelled as
        // zeros; no pixel reaches them, since the march's colour is
        // `(byte) >> 1`, at most 127.
        for entry in 240..=255u8 {
            assert_eq!(pal.entry(entry), [0, 0, 0], "the overrun at {entry}");
        }
    }

    // ------------------------------------------------------------ the part --

    /// A rig with the real player, `MUSIC1` seeked to order 91. The part sets
    /// no copper, so a step is the engine's poll and the part's `frame`.
    fn real_rig() -> Rig<PlayerMusic> {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(91);
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(PlayerMusic::new(player, 0))
    }

    #[test]
    fn the_entry_pre_roll_lands_the_gate_on_order_93_row_zero() {
        let mut rig = real_rig();
        // The player is fresh from `_zplaysong(91)`: no poll has advanced it.
        assert_eq!(rig.player().position().order, 0);
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        let position = rig.player().position();
        assert_eq!(position.order, 91);
        assert_eq!(position.row, 63, "527 polls leave the published row at 63");
        assert_eq!(rig.player().musplus(), -1, "the gate has not opened yet");
        // The engine's first tick polls once more: the 528th poll is the
        // order-93 row-0 crossing the reference's gate opens on.
        rig.player().poll();
        let position = rig.player().position();
        assert_eq!((position.order, position.row), (93, 0));
        assert_eq!(rig.player().musplus(), 0);
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_poll_and_blacks_the_inherited_screen() {
        let mut rig = real_rig();
        rig.surface.pixels_mut().fill(0xFF);
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player().position().order, 0, "nothing was polled");
        // The mode set and `inittwk` clear the screen the previous part left.
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
    }

    #[test]
    fn the_iterations_run_every_second_tick_and_the_frame_four_waits_add_two() {
        // The source's `frame += (frepeat = dis_waitb())`: two vblanks an
        // iteration, and the frame-4 body's two `dis_waitb` calls add two.
        let mut part = Coman::new();
        part.waves = Coman::read_waves().unwrap();
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
        let mut part = Coman::new();
        part.waves = Coman::read_waves().unwrap();
        let mut last_copy: Option<u16> = None;
        for k in 0..6usize {
            part.iteration(k as u32 * 2, -32);
            // The page this iteration copied into.
            let copy = if part.flip == 0 { PAGE_LO } else { PAGE_HI };
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
        let mut rig = real_rig();
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        // The engine's tick 0 is the 528th poll: the gate passes and the
        // first iteration runs, but the page it displays is the cleared one.
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert!(part.started);
        assert_eq!(part.start_page, PAGE_LO);
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
        // The wait tick shows the same page; the second iteration selects the
        // page the first copied into.
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.start_page, PAGE_LO);
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.start_page, PAGE_HI);
        // The picture is still below the screen: `startrise` starts at 160 and
        // falls one a vblank, so the copy's top row 70 + startrise is off the
        // bottom for the first ~90 vblanks.
        assert!(rig.surface.pixels()[..SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize]
            .iter()
            .all(|&pixel| pixel == 0));
        for _ in 3..80 {
            rig.step(&mut part);
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
        let mut rig = real_rig();
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        let mut tick = 0u32;
        loop {
            assert!(tick < FRAME_CAP, "the part outlived the dead frame cap");
            if rig.step(&mut part) == PartStatus::Finished {
                break;
            }
            tick += 1;
        }
        let musplus = rig.player().musplus();
        let position = rig.player().position();
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
        let mut rig = real_rig();
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(rig.step(&mut part), PartStatus::Finished, "sticky");
    }

    #[test]
    fn the_terrain_rises_off_the_bottom_before_the_break() {
        // Order 96 is `zplus == 1`, so from published row 35 the inner body
        // raises `startrise` a vblank; by tick 1,960 the 140-row copy starts
        // below row 200 and the screen is one colour (the reference's 17,000
        // is flat black, `reference-indices.md` §16). The part keeps running
        // off-screen until the break at tick 2,014.
        let mut rig = real_rig();
        let mut part = Coman::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        for tick in 0..=1_964u32 {
            rig.step(&mut part);
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
}
