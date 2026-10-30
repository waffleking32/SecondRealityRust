//! The bars: `mode16`, the four bar wipes (`KOE.C:236-316`), `waitborder`'s
//! 16-block palette slide and `doit1/2/3` (`KOE.C:317-577`).

use sr_engine::part::{PartCtx, PartStatus};

use super::{
    Phase, Techno, PLANE_BYTES, PLANE_ROW, PLANE_ROW_WIDE, SCREEN_WIDTH, TROLL_HEIGHT, TROLL_ROW,
};
use crate::parts::mode_view;
use crate::parts::sin1024::SIN1024;

/// One 320×200 page in plane bytes.
const PAGE_BYTES: usize = 8_000;
/// `vram = 0xa0000000 + 0x2000000*plv`: 0x200 paragraphs = 8,192 plane bytes.
const PAGE_STEP: usize = 0x2000;
/// `CRTC 0Ch = plv*0x20`: the start-address high byte for a page.
const PAGE_HIGH: i32 = 0x20;
/// `KOE.C:328-330`: `doit1(70*6)`, `doit2(70*12)`, `doit3(70*14)`.
const DOIT1_COUNT: i32 = 70 * 6;
const DOIT2_COUNT: i32 = 70 * 12;
const DOIT3_COUNT: i32 = 70 * 14;
/// `KOE.C:496`: doit3's `getmframe > 2520` break.
const DOIT3_MFRAME: i32 = 2520;
/// `KOE.C:501`: the countdown begins below 333.
const DOIT3_COUNTDOWN: i32 = 333;

impl Techno {
    /// `KOE.C:236-273`: the 16-colour mode set, the clear, the white fill and
    /// the dead `ip` build.
    pub(super) fn mode16(&mut self) {
        self.stride = PLANE_ROW;
        self.display_start = 0;
        self.pan = 0;
        // `memset(vram,0,64000)` (`KOE.C:259`): the last 1,536 bytes of each
        // plane keep what the interference left.
        for plane in &mut self.planes {
            plane[..64_000].fill(0);
            plane[..PAGE_BYTES].fill(0xFF);
        }
        // `ip = flash(-2)` returns `pal1`; the loop writes it and nothing
        // uploads it.
        for a in 0..16usize {
            self.flash_pal[a * 3] = a as i16 * 6 / 2;
            self.flash_pal[a * 3 + 1] = a as i16 * 7 / 2;
            self.flash_pal[a * 3 + 2] = a as i16 * 8 / 2;
        }
    }

    /// The upload a `flash` computed last frame: the source's `dis_waitb`
    /// sits between the computation and the upload (`techno.md` §4.2).
    pub(super) fn flush_flash(&mut self) {
        if let Some(flash) = self.wipe_pending.take() {
            self.flash_apply(flash);
        }
    }

    /// One iteration of a bar wipe's a-loop (`for(a=256;a>-400;a-=32)`): the
    /// band's next rows cleared, `flash(a)` left pending, and the wait after
    /// the 21st.
    pub(super) fn wipe_iteration(&mut self) {
        let a = 256 - self.wipe_step * 32;
        let zly = self.wipe_zy;
        self.wipe_zya += 1;
        self.wipe_zy += self.wipe_zya;
        let band = self.wipe_band;
        let mut v = zly * 40;
        for _ in zly..self.wipe_zy {
            for k in 0..10usize {
                let at = (v as usize + band * 10 + k) & (PLANE_BYTES - 1);
                for plane in &mut self.planes {
                    plane[at] = 0;
                }
            }
            v += 40;
        }
        self.wipe_pending = Some(a.max(0));
        self.wipe_step += 1;
        if self.wipe_step >= 21 {
            self.phase = Phase::WipeRowWait;
        }
    }

    /// A band's reset and its first a-iteration, which runs in the frame the
    /// preceding wait or `flash(256)` ends.
    fn start_band(&mut self) {
        self.wipe_zy = 0;
        self.wipe_zya = 0;
        self.wipe_step = 0;
        self.phase = Phase::Wipe;
        self.wipe_iteration();
    }

    /// `KOE.C:277-281`: wait for `musrow & 7 == 7`, then start the first bar.
    pub(super) fn row_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.flush_flash();
        if ctx.musrow() & 7 != 7 {
            return PartStatus::Continuing;
        }
        self.wipe_band = 0;
        self.wipe_flash = 0;
        self.start_band();
        self.present(ctx);
        PartStatus::Continuing
    }

    /// The wipe's flashes after its row wait; after `flash(256)` the next
    /// band starts, or doit1 after the fourth.
    pub(super) fn wipe_flash_tick(&mut self, ctx: &mut PartCtx) {
        self.flush_flash();
        if self.wipe_flash < 3 {
            self.wipe_pending = Some([64, 192, 256][usize::from(self.wipe_flash)]);
            self.wipe_flash += 1;
            return;
        }
        self.wipe_band += 1;
        if self.wipe_band < 4 {
            self.start_band();
            return;
        }
        // `doit1`'s first `waitborder` reads the row now and waits; the
        // loop's next frame uploads it.
        let row = ctx.musrow();
        self.bar_read(row);
        self.phase = Phase::Doit1;
        self.doit_first = true;
        self.doit_count = DOIT1_COUNT;
        self.rot = 45;
        self.vm = 50;
        self.vma = 0;
        self.plv = 0;
        self.pl = 1;
        self.page = 0;
    }

    /// `waitborder`'s upload half: the block the previous read armed, sliding
    /// down to block 0.
    fn upload_bar(&mut self) {
        let at = (self.curpal as usize) * 48;
        let block: [u8; 48] = self.bar_palette[at..at + 48].try_into().unwrap();
        self.upload_dac(0, &block);
        if self.curpal > 0 {
            self.curpal -= 1;
        }
    }

    /// `waitborder`'s read half: a row change to `& 7 == 7` arms block 15.
    fn bar_read(&mut self, row: i32) {
        if row != self.last_row {
            self.last_row = row;
            if row & 7 == 7 {
                self.curpal = 15;
            }
        }
    }

    /// `waitborder`, split across the frame: the upload, then the next read.
    fn bar_upload_and_read(&mut self, ctx: &mut PartCtx) {
        self.upload_bar();
        let row = ctx.musrow();
        self.bar_read(row);
    }

    /// One frame's eleven boxes about `centre`: `rot` picks the axes and `vm`
    /// scales the spacing (`KOE.C:355-379`, `:414-438`, `:509-524`).
    fn box_frame(&mut self, vm: i32, centre: (i32, i32)) {
        let sin = |at: i32| SIN1024[at as usize & 1023] as i32;
        let hx = sin(self.rot) * 16 * 6 / 5;
        let hy = sin(self.rot + 256) * 16;
        let vx = sin(self.rot + 256) * 6 / 5 * vm / 100;
        let vy = sin(self.rot + 512) * vm / 100;
        self.boxes(hx, hy, vx, vy, centre.0, centre.1);
    }

    /// doit2's and doit3's motion after a frame's boxes.
    fn doit2_motion(&mut self) {
        self.rot += self.rota / 10;
        self.vm += self.vma;
        if self.vm < 0 {
            self.vm -= self.vma;
            self.vma = -self.vma;
        }
        self.vma -= 1;
        self.rota += 1;
    }

    /// One frame of doit1 (`which = 1`) or doit2: eight pages a plane, the
    /// plane mask stepping after every eighth.
    pub(super) fn doit_tick(&mut self, ctx: &mut PartCtx, which: u8) -> PartStatus {
        self.bar_upload_and_read(ctx);
        if self.doit_first {
            self.doit_first = false;
            return PartStatus::Continuing;
        }
        self.doit_count -= 1;
        self.vbuf.fill(0);
        if which == 1 {
            self.box_frame(self.vm, (160, 100));
            self.rot += 2;
            self.vm += self.vma;
            if self.vm < 25 {
                self.vm -= self.vma;
                self.vma = -self.vma;
            }
            self.vma -= 1;
        } else {
            self.box_frame(self.vm / 64, (160, 100));
            self.doit2_motion();
        }
        self.blit(self.page, self.pl as u8, PLANE_ROW);
        self.display_start = (self.plv * PAGE_HIGH) << 8;
        self.plv = (self.plv + 1) & 7;
        self.page = self.plv as usize * PAGE_STEP;
        if self.plv == 0 {
            self.pl <<= 1;
            if self.pl > 15 {
                self.pl = 1;
            }
        }
        if self.doit_count <= 0 {
            if which == 1 {
                self.phase = Phase::Doit2;
                self.doit_count = DOIT2_COUNT;
                self.doit_first = true;
                self.rot = 50;
                self.rota = 10;
                self.vm = 100 * 64;
                self.vma = 0;
                self.plv = 0;
                self.pl = 1;
                self.page = 0;
            } else {
                self.phase = Phase::Doit3Prologue;
                self.doit_count = DOIT3_COUNT;
            }
        }
        self.present(ctx);
        PartStatus::Continuing
    }

    /// doit3's prologue tick: the bar upload, the mode set, then the loop's
    /// first pre-wait half.
    pub(super) fn doit3_prologue(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.bar_upload_and_read(ctx);
        self.dac[..48].fill(0);
        for plane in &mut self.planes {
            plane.fill(0);
        }
        self.display_start = 0;
        self.stride = PLANE_ROW_WIDE;
        self.pan = 0;
        self.rot = 45;
        self.rota = 10;
        self.rot2 = 0;
        self.vm = 100 * 64;
        self.vma = 0;
        self.xpos = 320;
        self.xposa = 0;
        self.repeat = 1;
        self.plv = 0;
        self.pl = 1;
        self.page = 40;
        if self.doit3_pre(ctx) {
            return self.doit3_finish(ctx);
        }
        self.phase = Phase::Doit3;
        self.present(ctx);
        PartStatus::Continuing
    }

    /// doit3's frame: the previous iteration's post-wait drawing, then the
    /// next iteration's pre-wait half.
    pub(super) fn doit3_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.upload_bar();
        self.doit_count -= 1;
        self.repeat = 1;
        self.doit3_post();
        if self.doit3_pre(ctx) {
            return self.doit3_finish(ctx);
        }
        self.present(ctx);
        PartStatus::Continuing
    }

    /// The post-wait half of a doit3 iteration: four pages, two apart.
    fn doit3_post(&mut self) {
        // The 16-colour screen's ATC pan is `xpos & 7` (`KOE.C:528`); with the
        // start at `xpos/8` plane bytes the offset is `xpos`.
        self.pan = (self.xpos & 7) as u8;
        self.vbuf.fill(0);
        self.box_frame(self.vm / 64, (self.doit_wx, self.doit_wy));
        self.doit2_motion();
        self.blit(self.page, self.pl as u8, PLANE_ROW_WIDE);
        self.display_start = (self.plv * PAGE_HIGH) << 8 | (self.display_start & 0xFF);
        self.plv = (self.plv + 2) & 7;
        self.page = self.plv as usize * PAGE_STEP + 40;
        if self.plv == 0 {
            self.pl <<= 1;
            if self.pl > 15 {
                self.pl = 1;
            }
        }
    }

    /// The pre-wait half of a doit3 iteration: the `getmframe` and countdown
    /// breaks, the box centre's spiral and the scroll. `true` when the loop
    /// breaks.
    fn doit3_pre(&mut self, ctx: &mut PartCtx) -> bool {
        if ctx.getmframe() > DOIT3_MFRAME {
            if self.xpos != 0 {
                self.xpos = 0;
            } else {
                return true;
            }
        }
        if self.doit_count < DOIT3_COUNTDOWN {
            while self.repeat > 0 {
                self.repeat -= 1;
                self.xpos -= self.xposa / 4;
                if self.xpos < 0 {
                    self.xpos = 0;
                } else {
                    self.xposa += 1;
                }
            }
            if self.xpos == 0 {
                return true;
            }
        }
        let sin = |at: i32| SIN1024[at as usize & 1023] as i32;
        if self.rot2 < 32 {
            self.doit_wx = sin(self.rot2) * self.rot2 / 8 + 160;
            self.doit_wy = sin(self.rot2 + 256) * self.rot2 / 8 + 100;
        } else {
            // C's `/4` truncates toward zero, not an arithmetic shift.
            self.doit_wx = sin(self.rot2) / 4 + 160;
            self.doit_wy = sin(self.rot2 + 256) / 4 + 100;
        }
        self.rot2 += 17;
        self.display_start = (self.display_start & 0xFF00) | (self.xpos / 8);
        let row = ctx.musrow();
        self.bar_read(row);
        false
    }

    /// `KOE.C:588-596`: mode 13h and `inittwk` clear the video memory and the
    /// DAC; the surface becomes the troll's interleaved four-plane image.
    fn doit3_finish(&mut self, ctx: &mut PartCtx) -> PartStatus {
        for plane in &mut self.planes {
            plane.fill(0);
        }
        self.dac = [0; 768];
        self.stride = TROLL_ROW;
        self.display_start = 0;
        self.pan = 0;
        self.page = 0;
        ctx.surface.pixels_mut().fill(0);
        *ctx.view = mode_view(SCREEN_WIDTH, TROLL_HEIGHT, Some(640), Some(0));
        self.phase = Phase::TrollWait1;
        self.present(ctx);
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_waitborder_slides_the_bar_blocks_and_resets_on_row_seven() {
        let mut t = Techno::new();
        t.bar_read(6);
        t.upload_bar();
        assert_eq!(t.curpal, 0, "a row that is not 7 holds block 0");
        t.bar_read(7);
        assert_eq!(t.curpal, 15);
        for i in 0..18usize {
            // The slide stops at block 0: the last three uploads are block 0.
            let block = 15usize.saturating_sub(i);
            let want = t.bar_palette[block * 48..block * 48 + 3].to_vec();
            t.upload_bar();
            assert_eq!(&t.dac[..3], &want[..], "upload {i} is block {block}");
        }
        assert_eq!(t.curpal, 0);
    }

    #[test]
    fn doit3s_pan_is_the_low_three_bits_of_xpos() {
        // The `(xpos & 3)*2` form is the troll's 256-colour one and would
        // lose bit 2 (`techno.md` §5).
        for xpos in [0i32, 1, 4, 5, 8, 13, 320] {
            let mut t = Techno::new();
            t.xpos = xpos;
            t.doit3_post();
            assert_eq!(t.pan, (xpos & 7) as u8, "xpos {xpos}");
        }
    }
}
