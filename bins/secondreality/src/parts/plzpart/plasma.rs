//! The plasma: `plz()` (`PLZ.C:62-122`) after `init_plz` (`:124-218`).
//!
//! `tw_opengraph2` (`TWEAK.ASM:44-67`) sets chain-4 off, byte mode, 400 rows
//! and a 96-byte plane row (`0x13 = 0x30`), and clears 64 KB. The upper view
//! starts at `cop_start = 96*(682-400)` (black memory) and the line compare
//! (`set_plzstart(60)`) splits to a lower view at plane byte 0, the 280 drawn
//! rows; `Split::scanline` is the register plus one (`reference-indices.md`
//! §13.2). `pompota` alternates the compare 60/61 and the pan register 4/0,
//! which a 256-colour mode shows as `(value & 7) / 2` = 2/0 pixels.
//!
//! `plzline` (`ASMYT.ASM:26-72`) is the index expression `setplzparas`
//! (`:74-95`) patches into its code, for byte `r` of line `y` with parameters
//! `(c1, c2, c3, c4)`:
//!
//! ```text
//!   b  = lsini16[c2 + 320 - 4r + y]
//!   px = psini[c1 + 8r + b]
//!   d  = lsini4[c4 + 16r + y]
//!   py = psini[c3 + 320 - 4r + d + 2y]
//!   value = (px + py) as u8          ; an 8-bit add, wrapping
//! ```

use sr_engine::part::PartCtx;
use sr_engine::view::{Split, View};
use sr_engine::Palette;

use super::copper::{CopperState, PalSource};
use super::tables::Tables;
use super::Plzpart;

/// `TWEAK.ASM:59-66`: CRTC 0x13 = 0x30 → a 96-byte plane row.
pub(super) const PLASMA_ROW_BYTES: usize = 96;
/// `PLZ.C:176`: `cop_start = 96*(682-400)` plane bytes.
pub(super) const PLASMA_COP_START: u32 = 96 * (682 - 400);
/// The engine's pixel stride for a 96-byte plane row: four pixels a byte.
const PLASMA_STRIDE: u16 = (PLASMA_ROW_BYTES * 4) as u16;
/// `PLZ.C:11`: `MAXY 280` — the plasma's drawn rows.
const PLASMA_ROWS: usize = 280;
/// The screen is 320×400 (`reference-indices.md` §13).
pub(super) const PLASMA_WIDTH: u16 = 320;
pub(super) const PLASMA_HEIGHT: u16 = 400;
/// `PLZ.C:46`: the timetable, in `getmframe` ticks. The last non-zero entry is
/// dead (the exit fires first); the trailing 0 is the array's terminator.
pub(super) const TIMETABLE: [i32; 6] = [723, 1491, 1875, 2259, 2778, 0];
/// `PLZ.C:94`'s `curpal == 5` is the release's 4 (image `0x1181c`): the plasma
/// exits 65 ticks after the **third** switch, by boot 8340 (A22, D24).
pub(super) const EXIT_PALETTE_RELEASE: i32 = 4;
/// `PLZ.C:46`'s `inittable` — the eight parameters each switch restores.
const INITTABLE: [[i32; 8]; 6] = [
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 4000, 4000, 1500, 2300, 3900, 1670],
    [3500, 1000, 3000, 1000, 3500, 3300, 2900, 2670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
    [1000, 2000, 3000, 4000, 3500, 2300, 3900, 3670],
];

/// The parameter walks a fresh run makes before tick 0. `MAIN.C` registers the
/// copper before `plz()` and `COP_PLZ` starts at 1 (`COPPER.ASM:19`), so every
/// retrace of the music wait walks `k`/`l`; the reference's 6000 is the initial
/// parameters walked 345 times at tick 277, so the wait was 68 ticks (the
/// best fit, rediff's `plzpart/plasma` 6000; A24). A fresh run seeks past the
/// wait, so the walks stand in for it.
pub(super) const PLASMA_WAIT_WALKS: u32 = 68;
/// `plzline` (`ASMYT.ASM:26-72`): 84 bytes for row `y`, four bytes a dword,
/// with the four parameter words `c1..c4`.
///
/// The 21 dwords run four bytes past the 320-pixel row into the 96-byte
/// stride's gap (`:41-69`); the port keeps all 84.
fn plasma_line(tables: &Tables, y: usize, c1: i32, c2: i32, c3: i32, c4: i32) -> [u8; 84] {
    let mut out = [0u8; 84];
    for (r, value) in out.iter_mut().enumerate() {
        *value = tables.plasma_value(r as i32, y as i32, c1, c2, c3, c4);
    }
    out
}

/// Four pixels a plane byte: the plasma's `View` stride and its lower view.
fn plasma_lower_view() -> View {
    let mut view = View::new(PLASMA_WIDTH, PLASMA_HEIGHT);
    view.y_line_offset = Some(PLASMA_STRIDE);
    view.border = Some(0);
    view
}

/// Write the CRTC state into the view, when the plasma is the live screen.
pub(super) fn apply_plasma_view(state: &CopperState, ctx: &mut PartCtx) {
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

impl Plzpart {
    /// `PLZ.C:124-218` `init_plz`, after the gate: the mode set, the display
    /// state, the three copper routines and the five palette tables.
    pub(super) fn init_plz(&mut self, ctx: &mut PartCtx) {
        // `TWEAK.ASM:44-67` `tw_opengraph2`: chain-4 off and the 64 KB clear
        // (all four planes). The surface is the four-plane image.
        ctx.surface.pixels_mut().fill(0);
        // `PLZ.C:179`: `for(a=0;a<256;a++) tw_setrgbpalette(a,63,63,63)`.
        ctx.set_palette(Palette::from_six_bit(&[63; 768]));
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

    /// `PLZ.C:76-118`: one pass of the plasma's loop. `true` when it breaks.
    pub(super) fn plasma_tick(&mut self, ctx: &mut PartCtx) -> bool {
        let published = ctx.getmframe();
        let mut state = self.copper.borrow_mut();
        // `tim+=frame_count; frame_count=0; count++;`
        state.frame_count = 0;
        if state.ttptr < TIMETABLE.len() && published > TIMETABLE[state.ttptr] {
            state.clear_colour_half();
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::plzpart::tables::tests::tables;

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
        assert_eq!(wrapped, plasma_line(&t, 0, 3500, 2, 3900, 3670)[83]);
        // The four parameters reach the expression.
        assert_ne!(out, plasma_line(&t, 7, 1000, 2000, 3000, 4000));
    }

    /// The plasma through the engine with the real player: the timetable's
    /// switches and the release's exit land on the reference's ticks (the
    /// trace's `zframe` crossings at boots 6698, 7733 and 8250, the plasma
    /// screen gone by 8340), the dead `timetable[4]` is never reached, and each switch's
    /// `memset(fadepal,0,768)` (`bugs.md` §1) clears only the colour half.
    #[test]
    fn the_plasma_switches_and_exits_on_the_reference_ticks() {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;
        use sr_engine::{Engine, IndexedFrame, View, MAX_HEIGHT, MAX_WIDTH};

        use super::super::Phase;

        let player = test_player(ModuleId::Music1).expect("MUSIC1 loads");
        lock(&player).play_song(62);
        let mut engine = Engine::new(MAX_WIDTH, MAX_HEIGHT, View::new(320, 200));
        engine.set_music(Box::new(PlayerMusic::new(player, 0)));
        let mut part = Plzpart::new();
        engine.start(&mut part).expect("the part starts");
        let mut frame = IndexedFrame::new(engine.view().width, engine.view().height);
        let mut switches = Vec::new();
        let (mut exit, mut mode_set) = (None, None);
        for index in 0..2700u32 {
            let before = part.copper.borrow().fadepal;
            let ttptr = part.copper.borrow().ttptr;
            engine.tick(&mut part, &mut frame);
            let state = part.copper.borrow();
            if state.ttptr != ttptr {
                switches.push(index);
                if ttptr == 1 {
                    assert!(
                        before.iter().any(|acc| acc & 0xFF00 != 0)
                            && before.iter().any(|acc| acc & 0x00FF != 0),
                        "the first fade left colour and fraction bytes to tell apart"
                    );
                }
                for (at, (old, new)) in before.iter().zip(state.fadepal.iter()).enumerate() {
                    assert_eq!(*new, old & 0x00FF, "switch {index}, accumulator {at}: colour zeroed, fraction kept");
                }
            }
            if exit.is_none() && part.phase == Phase::PlasmaEndWait {
                exit = Some(index);
            }
            if mode_set.is_none() && part.phase == Phase::CubeGate {
                mode_set = Some(index);
            }
        }
        assert_eq!(switches, [975, 2010, 2527], "boots 6698, 7733 and 8250");
        // The exit test fires 64 drop steps after the third switch; the next
        // tick's `frame_count` ends `PLZ.C:119`'s wait and sets the cube's mode.
        assert_eq!(exit, Some(2591));
        assert_eq!(mode_set, Some(2592), "boot 8315, 65 ticks after the third switch");
    }
}
