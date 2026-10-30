//! The troll (`KOE.C:588-727`): the order wait, the pan, the ripple and the
//! final wait, on the 320×400 screen whose picture is the surface itself.

use sr_engine::part::{PartCtx, PartStatus};

use super::{Phase, Techno, SCREEN_WIDTH, TROLL_HEIGHT};
use crate::parts::sin1024::SIN1024;

impl Techno {
    /// `KOE.C:638-648`: hold until `order > 35 || (order == 35 && row > 48)`.
    pub(super) fn troll_order_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let (order, row) = ctx.music().map_or((0, 0), |music| {
            let position = music.position();
            (position.order, position.row)
        });
        if order > 35 || (order == 35 && row > 48) {
            self.xpos = 0;
            self.xposa = 0;
            self.doit_count = 300;
            self.phase = Phase::TrollPan;
            return self.troll_pan(ctx);
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// The ripple's start: from the pan's end at 320, or its 300th frame.
    fn start_ripple(&mut self) {
        self.ripple = 0;
        self.ripplep = 8;
        self.ripple_c = 0;
        self.doit_count = 50;
        self.phase = Phase::TrollRipple;
    }

    /// `KOE.C:649-673`: the 300-count pan in from the left.
    pub(super) fn troll_pan(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.xpos == 320 {
            self.start_ripple();
            return self.troll_ripple(ctx);
        }
        self.xpos += self.xposa / 4;
        if self.xpos > 320 {
            self.xpos = 320;
        } else {
            self.xposa += 1;
        }
        self.troll_view(ctx);
        self.doit_count -= 1;
        if self.doit_count <= 0 {
            self.start_ripple();
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:674-703`: the 50-frame ripple and its sixteen fade steps.
    pub(super) fn troll_ripple(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if self.ripplep > 1023 {
            self.ripplep = 1024;
        } else {
            self.ripplep = self.ripplep * 5 / 4;
        }
        self.xpos = 320 + (SIN1024[self.ripple as usize & 1023] as i32) / self.ripplep;
        self.ripple += self.ripplep + 100;
        if self.ripple_c < 16 {
            let at = self.ripple_c as usize * 768;
            let fade: [u8; 768] = self.palfade[at..at + 768].try_into().unwrap();
            self.set_pal_area(&fade, 0, 256);
            self.ripple_c += 1;
        }
        self.troll_view(ctx);
        self.doit_count -= 1;
        if self.doit_count <= 0 {
            let palette = self.troll_palette;
            self.set_pal_area(&palette, 0, 256);
            self.xpos = 320;
            self.doit_count = 420;
            self.phase = Phase::TrollFinalWait;
        }
        self.troll_compose(ctx);
        self.present(ctx);
        PartStatus::Continuing
    }

    /// `KOE.C:705-726`: the final music wait at the picture's held position,
    /// `-6 < musplus < 16` or 420 frames.
    pub(super) fn troll_final_wait(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let musplus = ctx.musplus();
        let done = (musplus > -6 && musplus < 16) || {
            self.troll_view(ctx);
            self.doit_count -= 1;
            self.doit_count <= 0
        };
        if done {
            self.phase = Phase::Done;
        }
        self.troll_compose(ctx);
        self.present(ctx);
        if done {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }

    /// The troll's CRTC state: start low = xpos/4 plane bytes, the ATC pan
    /// `(xpos & 3)*2`, which the 256-colour mode halves.
    fn troll_view(&mut self, ctx: &mut PartCtx) {
        self.display_start = self.xpos / 4;
        self.pan = ((self.xpos & 3) * 2) as u8;
        ctx.view.start_addr = (self.display_start * 4) as u32;
        ctx.view.x_offset = 0;
        ctx.view.pan = ((self.pan as u32 & 7) / 2) as u8;
        ctx.view.y_line_offset = Some(640);
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = TROLL_HEIGHT;
    }

    /// The picture as the emulator draws the interleaved planes `lineblit`
    /// fills (`KOE.C:614-636`, at `vram + 80 + y*160`): 320 bytes a row from
    /// pixel 320 of a 640-pixel surface row.
    fn troll_compose(&self, ctx: &mut PartCtx) {
        let surface = ctx.surface.pixels_mut();
        for (y, row) in self.troll_pixels.chunks_exact(320).enumerate() {
            let at = y * 640 + 320;
            surface[at..at + 320].copy_from_slice(row);
        }
    }
}
