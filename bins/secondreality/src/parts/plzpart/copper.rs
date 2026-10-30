//! `COPPER.ASM`: the three copper routines and the state they share with the
//! part's frame loop, and the palette tables they fade through.
//!
//! `init_copper` (`:44-65`) registers `copper1` on the DIS's slot 1, `copper2`
//! on slot 2 and the empty `copper3` on slot 0. The DIS numbers them `0 = after
//! display start, 1 = just before retrace, 2 = in the retrace`
//! (`DIS/DISINT.ASM:284-291`) and the engine runs 0 → bands → 1 → 2, so the
//! mapping is the identity:
//!
//! ```text
//! engine slot 0  <- copper3  empty
//! engine slot 1  <- copper1  the CRTC start/pan writer
//! engine slot 2  <- copper2  frame_count++, the palette upload,
//!                            pompota + moveplz, do_drop
//! ```
//!
//! The engine composes a tick's bands before slot 2 runs, so slot 2's state is
//! seen from the next tick on, as the original's retrace is. `pompi` therefore
//! starts at 1, the source's BSS 0 plus the retrace before the first composed
//! frame.

use std::rc::Rc;

use sr_engine::part::PartCtx;
use sr_engine::Palette;

use super::plasma::{apply_plasma_view, PLASMA_COP_START};
use super::tables::Tables;
use super::Plzpart;

/// Build `pals[6][768]` exactly as `init_plz` does (`PLZ.C:181-217`). Each
/// table is runs of `(r, g, b)` from entry 1 on (entry 0 stays zero).
pub(super) fn build_pals(tables: &Tables) -> [[i16; 768]; 6] {
    let p = |a: usize| tables.ptau[a];
    let mut pals = [[0i16; 768]; 6];
    // Appends `rgb(a)` for each `a` of each run, from entry 1.
    type Run<'a> = (std::ops::Range<usize>, &'a dyn Fn(usize) -> [i16; 3]);
    let fill = |pal: &mut [i16; 768], runs: &[Run]| {
        let mut at = 3usize;
        for (range, rgb) in runs {
            for a in range.clone() {
                pal[at..at + 3].copy_from_slice(&rgb(a));
                at += 3;
            }
        }
    };
    // "RGB".
    fill(&mut pals[0], &[
        (1..64, &|a| [p(a), p(0), p(0)]),
        (0..64, &|a| [p(63 - a), p(0), p(0)]),
        (0..64, &|a| [p(0), p(0), p(a)]),
        (0..64, &|a| [p(a), p(0), p(63 - a)]),
    ]);
    // "RB-black".
    fill(&mut pals[1], &[
        (1..64, &|a| [p(a), p(0), p(0)]),
        (0..64, &|a| [p(63 - a), p(0), p(a)]),
        (0..64, &|a| [p(0), p(a), p(63 - a)]),
        (0..64, &|a| [p(a), p(63), p(a)]),
    ]);
    // "RB-white".
    fill(&mut pals[3], &[
        (1..64, &|a| [p(a), p(0), p(0)]),
        (0..64, &|a| [p(63), p(a), p(a)]),
        (0..64, &|a| [p(63 - a), p(63 - a), p(63)]),
        (0..64, &|_| [p(0), p(0), p(63)]),
    ]);
    // "white".
    fill(&mut pals[2], &[
        (1..64, &|_| [p(0) / 2; 3]),
        (0..64, &|a| [p(a) / 2; 3]),
        (0..64, &|a| [p(63 - a) / 2; 3]),
        (0..64, &|_| [p(0) / 2; 3]),
    ]);
    // "white II".
    fill(&mut pals[4], &[
        (1..75, &|a| [p(63 - a * 64 / 75); 3]),
        (0..106, &|_| [0; 3]),
        (0..75, &|a| {
            let v = p(a * 64 / 75);
            [v * 8 / 10, v * 9 / 10, v]
        }),
    ]);
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

/// Where the copper's pending palette upload reads from (`cop_pal`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum PalSource {
    Fade,
    Cube,
}

/// Everything the three copper routines own.
///
/// `PLZ.C` reads `cop_drop`, `curpal` and the parameter words from the same
/// globals `COPPER.ASM` writes, so the part and the callbacks share one state.
pub(super) struct CopperState {
    /// `PLZ.C:44`: `pals[6][768]`.
    pub(super) pals: [[i16; 768]; 6],
    /// `COPPER.ASM:175-181`: the drop's line-compare table.
    pub(super) dtau: [u16; 65],
    /// `COPPER.ASM:184`: 768 16-bit accumulators, colour in the high byte.
    pub(super) fadepal: [u16; 768],
    /// The table the fade accumulates (`cop_fadepal`).
    pub(super) fade_index: usize,
    /// `PLZ.C:45`: the next `pals` table to fade to.
    pub(super) curpal: i32,
    /// `PLZ.C:47`: the timetable index.
    pub(super) ttptr: usize,
    /// `COPPER.ASM:14`: the drop's step.
    pub(super) cop_drop: u16,
    /// `COPPER.ASM:19`: the plasma's copper routines are live.
    pub(super) cop_plz: bool,
    /// `COPPER.ASM:12`: the palette upload pending.
    pub(super) do_pal: bool,
    /// Whether that upload reads `fadepal` or the cube's `fpal`.
    pub(super) pal_source: PalSource,
    /// `COPPER.ASM:17`: the CRTC start (`cop_start`).
    pub(super) cop_start: u32,
    /// `COPPER.ASM:18`: the attribute pan register (`cop_scrl`).
    pub(super) cop_scrl: u8,
    /// `COPPER.ASM:122`: the split alternation's counter. **1, not 0**: the
    /// engine composes a tick before slot 2 runs, so the source's BSS 0 needs
    /// one retrace of phase to match the reference's frames.
    pub(super) pompi: u8,
    /// `PLZ.C:49-53`: the live and initial plasma parameters.
    pub(super) l: [i32; 4],
    pub(super) k: [i32; 4],
    pub(super) il: [i32; 4],
    pub(super) ik: [i32; 4],
    /// `COPPER.ASM:13`: the frame counter the copper increments.
    pub(super) frame_count: u32,
    /// The CRTC 0x18 line-compare register (8 bits plus the 8th bit).
    pub(super) line_compare: u16,
    /// Whether `tw_opengraph2` has put the plasma's mode on screen; the copper
    /// runs before that (during the wait) without touching the view.
    pub(super) plasma_mode: bool,
    /// `VECT.C:36`: the cube's shaded palette, 768 bytes.
    pub(super) fpal: [u8; 768],
    /// The CRTC state the beam last saw (copper1's write, for the cube's
    /// part-composed window).
    pub(super) display_start: u32,
    pub(super) display_pan: u8,
}

impl CopperState {
    /// `memset(fadepal,0,768)` (`PLZ.C:81`) zeroes the first 768 bytes of the
    /// 1,536-byte `fadepal`: the colour half, the accumulators' high bytes. The
    /// fractions (the low bytes, `COPPER.ASM:184`) stand, so the next table's
    /// fade starts from the last one's fractional state: the palette leak
    /// (`bugs.md` §1).
    pub(super) fn clear_colour_half(&mut self) {
        for acc in self.fadepal.iter_mut() {
            *acc &= 0x00FF;
        }
    }

    pub(super) fn new() -> CopperState {
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
    /// The DAC keeps six bits of each byte.
    fn upload_palette(&self) -> Palette {
        match self.pal_source {
            // `rep outsb` sends the colour half: the accumulators' high bytes.
            PalSource::Fade => Palette::from_six_bit(&self.fadepal.map(|acc| (acc >> 8) as u8)),
            PalSource::Cube => Palette::from_six_bit(&self.fpal),
        }
    }

    /// `COPPER.ASM:126-143`: the split/pan alternation.
    pub(super) fn pompota(&mut self) {
        self.cop_scrl = 4;
        self.line_compare = 60;
        self.pompi = self.pompi.wrapping_add(1);
        if self.pompi & 1 != 0 {
            self.cop_scrl = 0;
            self.line_compare = 61;
        }
    }

    /// `COPPER.ASM:145-173`: the parameter walk.
    pub(super) fn moveplz(&mut self) {
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
    pub(super) fn do_drop(&mut self) {
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

impl Plzpart {
    /// `COPPER.ASM:44-65` `init_copper`: the three registrations.
    pub(super) fn install_copper(&mut self, ctx: &mut PartCtx) {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn do_drops_table_and_split_walk_are_the_assemblys() {
        let dtau = build_dtau();
        assert_eq!(dtau[0], 60);
        assert_eq!(dtau[2], 60);
        assert_eq!(dtau[40], 194);
        assert_eq!(dtau[64], 404);
        // The split is the register + 1; 404 walks it off the 400-row screen.
        assert!(dtau[64] + 1 > super::super::plasma::PLASMA_HEIGHT);
    }

    /// `bugs.md` §1: `memset(fadepal,0,768)` clears the colour half and leaves
    /// the fractions, so the next fade's first sum carries the old fraction
    /// into the colour. A clean (16-bit) clear would start every channel at
    /// the step alone.
    #[test]
    fn a_table_switch_zeroes_the_colour_half_and_keeps_the_fraction() {
        let mut state = CopperState::new();
        state.fadepal[0] = 0x2AF0;
        state.fadepal[1] = 0x3F01;
        state.fadepal[767] = 0xFF80;
        state.clear_colour_half();
        assert_eq!(
            [state.fadepal[0], state.fadepal[1], state.fadepal[767]],
            [0x00F0, 0x0001, 0x0080],
            "the high (colour) byte is zeroed and the low (fraction) byte stands"
        );
        // The next table's first fade step: 0xF0 + 0x20 carries into the
        // colour byte, so the channel reads 1 where a cleared fraction reads 0.
        state.pals[0][0] = 0x0020;
        state.fade_index = 0;
        state.cop_drop = 127;
        state.do_drop();
        assert_eq!(state.cop_drop, 128);
        assert_eq!(state.fadepal[0], 0x0110, "the leftover fraction carried into the colour");
        assert_eq!(state.fadepal[0] >> 8, 1);
        // An accumulator with no leftover fraction starts from the step alone.
        state.pals[0][2] = 0x0020;
        state.do_drop();
        assert_eq!(state.cop_drop, 129);
        assert_eq!(state.fadepal[2], 0x0020);
    }

    /// `bugs.md` §17.2: the drop's 65th step resets the line compare and
    /// accumulates nothing, so the last table's fade is cut short.
    #[test]
    fn the_last_drop_step_resets_the_split_and_adds_nothing() {
        let mut state = CopperState::new();
        state.pals[0] = [0x0040; 768];
        state.fade_index = 0;
        state.fadepal[0] = 0x0010;
        state.cop_drop = 64;
        state.do_drop();
        assert_eq!(state.cop_drop, 65);
        assert_eq!(state.line_compare, 400, "the split is reset");
        assert_eq!(state.fadepal[0], 0x0010, "nothing is accumulated on the 65th step");
        // The next step does accumulate (66..=96 is the fade).
        state.do_drop();
        assert_eq!(state.cop_drop, 66);
        assert_eq!(state.fadepal[0], 0x0050);
    }

    #[test]
    fn the_split_and_pan_alternate_at_the_reference_parity() {
        // `pompi` starts at 1, so the first retrace writes 60 / pan 4, then 61 / 0.
        let mut state = CopperState::new();
        let registers: Vec<(u16, u8)> = (0..4)
            .map(|_| {
                state.pompota();
                (state.line_compare, state.cop_scrl)
            })
            .collect();
        assert_eq!(registers, [(60, 4), (61, 0), (60, 4), (61, 0)]);
        // Tick 277 (boot 6000) is composed after 277 slot-2 calls: register 60,
        // the split at scanline 61, the reference's first content row.
        let mut state = CopperState::new();
        for _ in 0..277 {
            state.pompota();
        }
        assert_eq!((state.line_compare, state.cop_scrl), (60, 4));
    }
}
