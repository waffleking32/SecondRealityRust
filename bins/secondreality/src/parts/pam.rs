//! PAM — the Praxis explosion: a 42-frame animation flipped between two pages.
//!
//! `PAM/OUTTAA.C` (67 lines) is the part, over `PAM/ASMYT.ASM`'s `ulosta_frame`
//! (the delta unpacker), `PAM/COPPER.ASM` (two copper routines, one of them a
//! bare `retf`), and the linked `OUT.ANI` the ASMYT reader consumes. Its acts:
//!
//! 1. `:34` — `dis_partstart`.
//! 2. `:36-37` — 63 washed copies of the palette, `pal[a*768+b] =
//!    (63*a + (64-a)*pal[b]) / 64`, each over the *base* copy 0.
//! 3. `:39` — the wait `while(dis_sync()<10 && !dis_exit());`.
//! 4. `:40-45` — `tw_waitvr`, the near-white palette copy 63, the mode set
//!    (which clears video memory), `init_uframe`, `init_copper`, and
//!    `frame_count=0`.
//! 5. `:46-64` — the 45-step loop: one `while(frame_count<4);` wait a step,
//!    one animation frame drawn into the alternate page and flipped to, and
//!    one palette copy from `wfade` queued for the next retrace.
//! 6. `:65` — `close_copper`.
//!
//! **The display.** `tw_opengraph` (`PAM/TWEAK.ASM:18-41`) is ALKU's unchained
//! mode without the CRTC offset write: 320×200, an 80-byte plane row
//! (`reference-indices.md` §1.6, §6). `ulosta_frame` writes a linear 64,000-byte
//! image, pixel `n` to plane `n & 3` byte `n >> 2`, which is surface pixel `n`;
//! `tw_setstart(16384)` is a plane-byte address, so page 1 starts at pixel
//! `0x4000 * 4`.
//!
//! **The two pages.** `OUT.ANI` frame `n` is a delta against `n-2`, because the
//! player alternates the two pages (`OUTTAA.C:51-60`); the port keeps both pages
//! in the surface and draws each frame into the off-screen one. The decoder is
//! `sr_assets::anim` (A3, A4).
//!
//! **The cadence and the anchor.** `copper2` increments `frame_count` once a
//! retrace (`COPPER.ASM:60-81`), so a step is four ticks. The anchor is 7710,
//! the reference's first flat-white frame, two copper frames into the first
//! wait ([`FRAME_COUNT_AT_ANCHOR`], `docs/fidelity/pam.md` §4). A run whose gate
//! is closed at its start keeps the predecessor's screen and runs the setup on
//! the tick the gate opens.
//!
//! **Preserved.** The FLI header counts 41 frames, the container 42, and the
//! loop draws 40 (`bugs.md` §11); steps 41-45 only re-queue copy 63, the closing
//! white. `ruudulle` (`ASMYT.ASM:207-320`) is dead and not ported. Measurements:
//! `docs/fidelity/pam.md`.

use std::cell::RefCell;
use std::rc::Rc;

use sr_assets::{load_frames, pal, Format};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::read_asset;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "pam";

// ------------------------------------------------------------- the screen --

/// `PAM/TWEAK.ASM:20` mode 13h, with chain-4 off: 320 pixels a row.
const VIEW_WIDTH: u16 = 320;
/// 200 rows, the mode's own default (0x13 is never written, §1.6).
const VIEW_HEIGHT: u16 = 200;
/// One plane row: `ANIM.C:44`'s 64,000 pixels over 200 rows over four planes.
const PLANE_ROW_BYTES: usize = 80;
/// Four displayed pixels a plane byte in the engine's interleaved surface.
const PIXELS_PER_BYTE: usize = 4;
/// The view's line stride in pixels: 80 plane bytes × 4.
const LINE_PIXELS: u16 = (PLANE_ROW_BYTES * PIXELS_PER_BYTE) as u16;
/// One page's pixels: `ANIM.C:44`'s `while(cnt<64000)`.
const PAGE_PIXELS: usize = 64_000;
/// `OUTTAA.C:53`/`:58`: `ulosta_frame(0x0a400)` and `tw_setstart(16384)`
/// against `0x0a000` and `0`. The start is a plane-byte address; this is the
/// port's pixel address, four per plane byte.
const PAGE_ORIGIN: [u32; 2] = [0, 0x4000 * PIXELS_PER_BYTE as u32];
/// The surface pixels the second page reaches: the surface must be at least
/// this long (it is 640×400 = 256,000).
const SURFACE_PIXELS: usize = PAGE_ORIGIN[1] as usize + PAGE_PIXELS;

// ------------------------------------------------------------- the assets --

/// `INCLUDE.ASM:9-26` links OUT.IN0-4; the port reads the same bytes from the
/// packed file the fragment source was made from.
const ANI_PATH: &str = "assets/PAM/OUT.ANI";
/// `ANIM.C:19-21` reads `pal.pal` and hands it to `tw_setpalette`.
const PAL_PATH: &str = "assets/PAM/PAL.PAL";
/// `ANIM.C:44` bounds every frame at 64,000; the container's record count is
/// the decoder's (42), one more than the FLI header's word (41).
const ANI_FRAMES: usize = 42;
/// `OUTTAA.C:49` `if(f<=40)`: the frames the loop actually unpacks.
const DRAWN_FRAMES: u32 = 40;

// -------------------------------------------------------------- the loop --

/// `OUTTAA.C:46` `f++<45`: the loop's own bound.
const LOOP_STEPS: u32 = 45;
/// `OUTTAA.C:48` `while(frame_count<4)`: four copper frames a step.
const TICKS_PER_STEP: u16 = 4;
/// `OUTTAA.C:39` `dis_sync()<10`: the start code the block-1 music reaches at
/// the anchor (`ordersync1`'s code 10 from order 13 on).
const SYNC_CODE: i32 = 10;
/// The `frame_count` the source's counter reads at boot 7710, so the first step
/// is 7712 (`docs/fidelity/pam.md` §4 derives it from the support frames).
const FRAME_COUNT_AT_ANCHOR: u16 = 2;

/// `OUTTAA.C:22-26` `wfade[100]`, with the 59 initialisers the source writes
/// and only the used range 1-45 ever indexed.
const WFADE: [u8; 59] = [
    63, 32, 16, 8, 4, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    1, 2, 4, 6, 9, 14, 20, 28, 37, 46, 56, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63, 63,
    63, 63, 63, 63, 63,
];
/// `OUTTAA.C:36-37` `for(a=1;a<64;a++)`: the 64 palette copies.
const PALETTE_COPIES: usize = 64;
/// The copy `tw_setpalette(&pal[768*63])` puts up before the loop.
const OPENING_COPY: usize = 63;

// --------------------------------------------------------------- the part --

/// `COPPER.ASM`'s `frame_count` and `cop_pal`/`do_pal`, in one place for the
/// slot 2 callback.
struct CopperState {
    /// `:10` `frame_count dw 0`, incremented once a retrace by `copper2`.
    frame_count: u16,
    /// `:11-12` `cop_pal`/`do_pal`: the copy the next retrace uploads.
    pending: Option<Palette>,
}

impl CopperState {
    /// The counter starts at zero and is set by [`Pam::setup`]: `init_copper`
    /// runs after the gate and zeroes it (`OUTTAA.C:44-45`).
    fn new() -> CopperState {
        CopperState {
            frame_count: 0,
            pending: None,
        }
    }

    /// `COPPER.ASM:60-81`: increment the counter, then upload the queued copy.
    /// The engine's slot 2 runs after the tick's bands, so the upload shows in
    /// the next tick's frame — the source's retrace upload, seen a frame later.
    fn slot2(&mut self, ctx: &mut PartCtx) {
        self.frame_count = self.frame_count.wrapping_add(1);
        if let Some(palette) = self.pending.take() {
            ctx.set_palette(palette);
        }
    }
}

/// Where the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `OUTTAA.C:39`: waiting for `dis_sync()` to reach code 10. The
    /// source's post-gate setup has not run yet, so the predecessor's screen
    /// is still up and no copper is installed.
    Sync,
    /// `OUTTAA.C:46-64`: the 45-step loop, at `f` steps taken.
    Loop,
    /// `close_copper` has run and the part has ended.
    Done,
}

/// PAM: the two pages, the 42 decoded frames and the 64 palette copies.
pub struct Pam {
    shared: Rc<RefCell<CopperState>>,
    /// The decoded animation: 42 frames of 64,000 pixels, the decoder's own
    /// two-page deltas already applied.
    frames: Vec<Vec<u8>>,
    /// Copy 0 is `PAL.PAL`; copies 1-63 are `OUTTAA.C:36-37`'s washes.
    palettes: Vec<Palette>,
    /// `OUTTAA.C:30`'s `f`, incremented before each step.
    f: u32,
    phase: Phase,
}

impl Pam {
    /// A part with nothing loaded: `init` reads the animation and its palette.
    pub fn new() -> Pam {
        Pam {
            shared: Rc::new(RefCell::new(CopperState::new())),
            frames: Vec::new(),
            palettes: Vec::new(),
            f: 0,
            phase: Phase::Sync,
        }
    }

    /// `OUTTAA.C:36-37`: 63 washed copies over the base copy 0.
    fn washes(base: &Palette) -> Vec<Palette> {
        let mut copies = Vec::with_capacity(PALETTE_COPIES);
        copies.push(*base);
        for a in 1..PALETTE_COPIES as u32 {
            let mut palette = Palette::new();
            for entry in 0..256u16 {
                let rgb = base.entry(entry as u8);
                palette.set_entry(
                    entry as u8,
                    [
                        ((63 * a + (64 - a) * u32::from(rgb[0])) / 64) as u8,
                        ((63 * a + (64 - a) * u32::from(rgb[1])) / 64) as u8,
                        ((63 * a + (64 - a) * u32::from(rgb[2])) / 64) as u8,
                    ],
                );
            }
            copies.push(palette);
        }
        copies
    }

    /// Read the animation and the palette, set the mode and the copper.
    fn prepare(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part waits on dis_sync() (PAM/OUTTAA.C:39) and the run has \
             no music to wait on",
        )?;
        let ani = read_asset(NAME, ANI_PATH)?;
        let frames = load_frames(&ani, Format::Ani)
            .map_err(|error| PartError::new(NAME, format!("{ANI_PATH}: {error}")))?;
        if frames.len() != ANI_FRAMES {
            return Err(PartError::new(
                NAME,
                format!(
                    "{ANI_PATH} holds {} frames; the container's record chain is {ANI_FRAMES}",
                    frames.len()
                ),
            ));
        }
        for (index, frame) in frames.iter().enumerate() {
            if (frame.width(), frame.height()) != (VIEW_WIDTH, VIEW_HEIGHT) {
                return Err(PartError::new(
                    NAME,
                    format!(
                        "{ANI_PATH} frame {index} is {}x{}; the animation is {VIEW_WIDTH}x{VIEW_HEIGHT}",
                        frame.width(),
                        frame.height()
                    ),
                ));
            }
        }
        self.frames = frames.into_iter().map(|frame| frame.pixels().to_vec()).collect();

        let pal_bytes = read_asset(NAME, PAL_PATH)?;
        let base = pal::decode(&pal_bytes)
            .map_err(|error| PartError::new(NAME, format!("{PAL_PATH}: {error}")))?;
        self.palettes = Pam::washes(&base);

        if ctx.surface.pixels().len() < SURFACE_PIXELS {
            return Err(PartError::new(
                NAME,
                format!(
                    "the part's second page reaches pixel {SURFACE_PIXELS} and the surface \
                     holds {}",
                    ctx.surface.pixels().len()
                ),
            ));
        }

        self.f = 0;
        // `OUTTAA.C:39-45`: the setup is after the gate. The anchored entry
        // run has it behind it, at the anchor's counter; a mid-chain part that
        // finds the gate open (its predecessor drove the player past it) runs
        // it now with the source's `frame_count=0`.
        if ctx.music().is_some_and(|music| music.sync() >= SYNC_CODE) {
            let frame_count = if ctx.music_fresh() {
                FRAME_COUNT_AT_ANCHOR
            } else {
                0
            };
            self.setup(ctx, frame_count);
            self.phase = Phase::Loop;
        } else {
            self.phase = Phase::Sync;
        }
        Ok(())
    }

    /// `OUTTAA.C:40-45`: `tw_waitvr` (folded into the frame count), the opening
    /// wash, `tw_opengraph`'s clear and mode, `init_uframe` and `init_copper`,
    /// leaving the counter at `frame_count` (the source's 0, or
    /// [`FRAME_COUNT_AT_ANCHOR`] for the anchored run).
    fn setup(&mut self, ctx: &mut PartCtx, frame_count: u16) {
        // `TWEAK.ASM:27-34`: the mode set clears both pages, the base frames 0
        // and 1 are decoded against.
        ctx.surface.clear(0);
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(LINE_PIXELS);
        ctx.view.set_origin(PAGE_ORIGIN[0]);
        ctx.view.pan = 0;
        ctx.view.split = None;
        // `OUTTAA.C:41` `tw_setpalette(&pal[768*63])`: the opening wash.
        ctx.set_palette(self.palettes[OPENING_COPY]);

        {
            let mut shared = self.shared.borrow_mut();
            *shared = CopperState::new();
            shared.frame_count = frame_count;
        }
        let slot2 = Rc::clone(&self.shared);
        // `COPPER.ASM:37-58`: `copper3` (a bare `retf`) in slots 0 and 1,
        // `copper2` in slot 2.
        ctx.set_copper(0, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(1, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(2, Box::new(move |ctx: &mut PartCtx| slot2.borrow_mut().slot2(ctx)));
    }

    /// `close_copper` (`COPPER.ASM:14-35`): no-ops in all three slots.
    fn close_copper(&mut self, ctx: &mut PartCtx) {
        ctx.set_copper(0, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(1, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(2, Box::new(|_ctx: &mut PartCtx| {}));
        self.phase = Phase::Done;
    }

    /// `OUTTAA.C:48-63`: one step of the loop, with `f` already incremented.
    fn step(&mut self, ctx: &mut PartCtx) {
        let f = self.f;
        if f <= DRAWN_FRAMES {
            let frame = (f - 1) as usize;
            let page = ((f - 1) & 1) as usize;
            let origin = PAGE_ORIGIN[page] as usize;
            ctx.surface.pixels_mut()[origin..origin + PAGE_PIXELS]
                .copy_from_slice(&self.frames[frame]);
            // `OUTTAA.C:53-59`: the flip is `tw_setstart` after the unpack.
            ctx.view.set_origin(PAGE_ORIGIN[page]);
        }
        // `OUTTAA.C:62-63`: `cop_pal=&pal[768*wfade[f]]; do_pal=1;` — uploaded
        // by slot 2 at the tick's retrace and shown from the next frame.
        let copy = usize::from(WFADE[f as usize]);
        self.shared.borrow_mut().pending = Some(self.palettes[copy]);
    }

    /// Whether the part has ended.
    #[cfg(test)]
    fn finished(&self) -> bool {
        self.phase == Phase::Done
    }
}

impl Part for Pam {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        self.prepare(ctx)
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `!dis_exit()` guards both loops (`OUTTAA.C:39`, `:46`): every tick.
        if self.phase == Phase::Done {
            return PartStatus::Finished;
        }
        if ctx.exit_requested {
            self.close_copper(ctx);
            return PartStatus::Finished;
        }
        if self.phase == Phase::Sync {
            let code = ctx.music().map_or(i32::MIN, |music| music.sync());
            if code < SYNC_CODE {
                return PartStatus::Continuing;
            }
            // `OUTTAA.C:40-45`: the setup tick is not a step.
            self.setup(ctx, 0);
            self.phase = Phase::Loop;
            return PartStatus::Continuing;
        }
        // `OUTTAA.C:48` `while(frame_count<4);`, a tick at a time.
        if self.shared.borrow().frame_count < TICKS_PER_STEP {
            return PartStatus::Continuing;
        }
        self.shared.borrow_mut().frame_count = 0;
        self.f += 1;
        self.step(ctx);
        if self.f >= LOOP_STEPS {
            // `OUTTAA.C:65`: the loop's last body ran; the copper goes.
            self.close_copper(ctx);
            return PartStatus::Finished;
        }
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{rgb_of, trace_position, Rig, ScriptedMusic};
    use crate::shell::{MusicStart, PAM_ANCHOR};
    use sr_engine::dac_expand;
    use sr_engine::music::sync_from;

    fn rig(music: ScriptedMusic) -> Rig {
        Rig::new(VIEW_WIDTH, VIEW_HEIGHT).with(music)
    }

    fn page_of(rig: &Rig, page: usize) -> Vec<u8> {
        let start = PAGE_ORIGIN[page] as usize;
        rig.surface.pixels()[start..start + PAGE_PIXELS].to_vec()
    }

    fn started_with(music: ScriptedMusic) -> (Rig, Pam) {
        let mut rig = rig(music);
        let mut part = Pam::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Pam) {
        started_with(ScriptedMusic::sync_codes(vec![SYNC_CODE]))
    }

    /// The tick a step lands on: the first step at tick 2 (boot 7712), then
    /// one every four ticks.
    fn step_tick(f: u32) -> u32 {
        FRAME_COUNT_AT_ANCHOR as u32 + 4 * (f - 1)
    }

    #[test]
    fn the_sync_gate_holds_until_code_ten_then_sets_up_and_waits_four_ticks() {
        // `OUTTAA.C:36-48`: a closed gate keeps the predecessor's screen; the
        // setup runs when it opens and the counter starts at zero.
        let (mut rig, mut part) = started_with(ScriptedMusic::sync_codes(vec![9, 9, 9, SYNC_CODE]));
        rig.surface.clear(7);
        rig.palette = Palette::new();
        rig.palette.set_entry(1, [1, 2, 3]);
        assert_eq!(part.phase, Phase::Sync);
        for _ in 0..3 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        }
        assert_eq!(part.f, 0, "no step while the gate is closed");
        assert!(
            rig.surface.pixels().iter().all(|pixel| *pixel == 7),
            "the predecessor's screen stays up"
        );
        assert_eq!(rig.palette.entry(1), [1, 2, 3], "and its DAC");
        // Tick 3 is code 10: the source's post-gate setup, `frame_count=0`,
        // and the loop's first wait frame — not a step.
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.f, 0, "the setup tick does not step");
        assert_eq!(rig.palette, part.palettes[OPENING_COPY], "the near-white setup palette");
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "the mode set cleared");
        // Four copper frames after the gate opens: ticks 4-6 read 1, 2 and 3.
        for tick in 4..=6 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing, "tick {tick}");
            assert_eq!(part.f, 0, "tick {tick} is still waiting");
        }
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.f, 1, "the first step is four ticks after the gate");
        assert_eq!(page_of(&rig, 0), part.frames[0], "the first step drew frame 0");
    }

    #[test]
    fn a_mid_chain_start_with_the_gate_open_starts_the_counter_at_zero() {
        // Mid-chain, an open gate means the predecessor drove the player past
        // it: the setup has not run, so the counter is the source's 0.
        let mut rig = rig(ScriptedMusic::sync_codes(vec![SYNC_CODE]));
        let mut part = Pam::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(part.phase, Phase::Loop, "the open gate starts the loop");
        assert_eq!(
            part.shared.borrow().frame_count, 0,
            "a mid-chain start is not the anchored 7710 start"
        );
    }

    #[test]
    fn four_ticks_a_step_and_forty_of_the_containers_frames_drawn() {
        // `OUTTAA.C:46-49`: 45 steps, four copper frames between them, and
        // `if(f<=40)` draws only the first 40 of the container's 42 frames.
        let (mut rig, mut part) = started();
        let mut drawn = Vec::new();
        let mut last_page: Option<Vec<u8>> = None;
        while rig.tick <= step_tick(LOOP_STEPS) + 4 {
            let before = part.f;
            rig.step(&mut part);
            if part.f != before {
                drawn.push(part.f);
                // The step's tick is the formula's.
                assert_eq!(rig.tick - 1, step_tick(part.f), "step {} landed late", part.f);
                if part.f <= DRAWN_FRAMES {
                    last_page = Some(page_of(&rig, ((part.f - 1) & 1) as usize));
                }
            }
            if part.finished() {
                break;
            }
        }
        assert_eq!(drawn, (1..=LOOP_STEPS).collect::<Vec<_>>(), "45 steps, in order");
        // The 40 drawn frames are exactly frames 0-39: frames 40 and 41 of
        // the container never reach a page.
        for page in 0..2 {
            let drawn = page_of(&rig, page);
            assert_ne!(drawn, part.frames[40], "frame 40 was drawn");
            assert_ne!(drawn, part.frames[41], "frame 41 was drawn");
        }
        // The last drawn page holds frame 39 (f = 40).
        assert_eq!(last_page.unwrap(), part.frames[39]);
        assert!(part.finished(), "the loop ended");
    }

    #[test]
    fn the_first_step_uses_wfade_one_half_wash_not_the_opening_sixty_three() {
        let (mut rig, mut part) = started();
        assert_eq!(rig.palette, part.palettes[OPENING_COPY], "the opening white");
        // Ticks 0 and 1 are the wait's last two frames; tick 2 is the step.
        for _ in 0..3 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 1);
        // Slot 2 uploaded the first step's copy.
        assert_eq!(rig.palette, part.palettes[32], "wfade[1] = 32, not 63");
        assert_ne!(part.palettes[32], part.palettes[63]);
        // Copy 63 is the near-white one: entry 15 is 62, widened 251.
        assert_eq!(part.palettes[63].entry(15), [62, 62, 62]);
        assert_eq!(dac_expand(62), 251);
    }

    #[test]
    fn the_flip_draws_into_the_off_screen_page_and_the_delta_base_is_two_back() {
        // `OUTTAA.C:51-60`: page 0 at step 1, page 1 at step 2, page 0 at
        // step 3; each page holds the frame two back until the new one is
        // unpacked into it.
        let (mut rig, mut part) = started();
        assert_eq!(page_of(&rig, 0), vec![0u8; PAGE_PIXELS], "the mode set cleared page 0");
        assert_eq!(page_of(&rig, 1), vec![0u8; PAGE_PIXELS], "and page 1");
        // Ticks 0 and 1 wait; tick 2 is step 1 into page 0, displayed.
        for _ in 0..3 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 1);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[0], "the flip shows page 0");
        assert_eq!(page_of(&rig, 0), part.frames[0]);
        assert!(page_of(&rig, 1).iter().all(|pixel| *pixel == 0), "page 1 is untouched");
        // Ticks 3-5 wait; tick 6 is step 2 into the off-screen page 1.
        for _ in 0..4 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 2);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[1], "the flip shows page 1");
        assert_eq!(page_of(&rig, 1), part.frames[1]);
        assert_eq!(page_of(&rig, 0), part.frames[0], "page 0 still holds frame 0");
        // Ticks 7-9 wait; tick 10 is step 3 into page 0, which held frame 0.
        for _ in 0..4 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 3);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[0]);
        assert_eq!(page_of(&rig, 0), part.frames[2], "frame 2 replaces frame 0");
        assert_ne!(part.frames[2], part.frames[0], "the decode is not a copy of two back");
        assert_eq!(page_of(&rig, 1), part.frames[1], "page 1 still holds frame 1");
    }

    #[test]
    fn exit_requested_while_waiting_ends_the_part_on_that_tick() {
        let (mut rig, mut part) = started();
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert!(part.finished());
        // `close_copper` left `retf`s: the counter stops and a queued palette
        // is never uploaded.
        part.shared.borrow_mut().pending = Some(part.palettes[0]);
        let counter = part.shared.borrow().frame_count;
        assert_eq!(rig.step(&mut part), PartStatus::Finished, "a finished part stays finished");
        assert_eq!(part.shared.borrow().frame_count, counter, "the counter stopped");
        assert!(part.shared.borrow().pending.is_some(), "no upload after close_copper");
        // The exit during the gate too.
        let (mut rig, mut part) = started_with(ScriptedMusic::sync_codes(vec![9]));
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert!(part.finished());
    }

    #[test]
    fn the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position() {
        // The block-1 clock starts at 400 and the run draws from 7710, so 7310
        // frames are pre-polled; the engine then polls once for the tick.
        let start = MusicStart::for_part("pam", Some(PAM_ANCHOR));
        assert_eq!(start.frame, PAM_ANCHOR);
        assert_eq!(start.waiting, 0);
        assert_eq!(start.pre, 7310, "7710 - 400");

        let player = crate::audio::test_player(crate::shell::ModuleId::Music0).unwrap();
        crate::audio::lock(&player).play_song(0);
        let state_after = |player: &crate::audio::Shared| {
            let locked = crate::audio::lock(player);
            let state = locked.state();
            ((state.order, state.row), locked.zframe())
        };
        let (mut at_7304, mut at_7305) = (None, None);
        for poll in 0..start.pre {
            if poll == 7304 {
                at_7304 = Some(state_after(&player));
            }
            if poll == 7305 {
                at_7305 = Some(state_after(&player));
            }
            crate::audio::lock(&player).poll();
        }
        let at_7310 = state_after(&player);
        crate::audio::lock(&player).poll(); // the engine's own tick's poll
        let at_7311 = state_after(&player);

        // The row changes on the trace's frame, 7705 (`pam.md` §5).
        assert_eq!(at_7304, Some(((13, 0), 5004)));
        assert_eq!(at_7305, Some(((13, 1), 5004)));
        assert_eq!(trace_position("second1b", 7705), (13, 1));
        // The trace's 7710 sample; the engine's tick poll is one further.
        assert_eq!(at_7310, ((13, 1), 5008));
        assert_eq!(at_7311, ((13, 1), 5009));
        let (position, _) = at_7311;
        assert_eq!(position, trace_position("second1b", PAM_ANCHOR));
        assert_eq!(sync_from(position.0, position.1), 10, "the gate's own code");
    }

    #[test]
    fn a_steps_palette_shows_one_tick_after_its_frame() {
        let (mut rig, mut part) = started();
        let mut frames = Vec::new();
        for _ in 0..4 {
            frames.push(rig.step_render(&mut part).1);
        }
        // The first step's tick still shows the opening copy 63; slot 2's
        // upload shows `wfade[1] = 32` on the next.
        assert_eq!(
            frames[2],
            rgb_of(&part.frames[0], &part.palettes[OPENING_COPY]),
            "the first step's tick still shows the opening copy 63"
        );
        assert_eq!(
            frames[3],
            rgb_of(&part.frames[0], &part.palettes[usize::from(WFADE[1])]),
            "the next tick shows wfade[1] = 32"
        );
        assert_ne!(frames[2], frames[3], "the palette moves one tick later");
    }
}
