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
//! # The display: 320×200, planar memory, two pages 0x4000 bytes apart
//!
//! `tw_opengraph` (`PAM/TWEAK.ASM:18-41`) is ALKU's mode set without the CRTC
//! offset write: mode 13h, chain-4 off, all four planes writable, CRTC 0x14 = 0
//! and 0x17 = `0xe3`. The screen is 320×200 with an 80-byte plane row
//! (`reference-indices.md` §1.6, §6).
//!
//! `ulosta_frame` writes a 64,000-byte *linear* image into that memory one
//! byte a pixel: pixel `n` goes to plane `n & 3` at plane-byte `n >> 2`, which
//! in the emulator's four-byte-group VGA memory is linear byte `n`
//! (`vga_memory.cpp`'s unchained handler; `reference-indices.md` §1.6). The
//! engine's surface is indexed the same way ALKU's is — pixel `4*plane_byte +
//! plane` — so page 0 is surface pixels 0..64,000 and page 1 is pixels
//! 0x10000..0x10000+64,000: the CRTC start `tw_setstart(16384)` is a
//! *plane-byte* address, and `View::set_origin` takes pixels, so the source's
//! 0x4000 is `0x4000 * 4` here, exactly as `ALKU`'s copper1 maps its own
//! plane-byte start.
//!
//! # The two-page flip, and the delta base it exists for
//!
//! `OUT.ANI` is 42 delta frames; frame `n` is packed against frame `n-2`, not
//! `n-1`, because the player alternates `ulosta_frame(0x0a000)` /
//! `tw_setstart(0)` with `ulosta_frame(0x0a400)` / `tw_setstart(16384)`
//! (`OUTTAA.C:51-60`): each frame is unpacked into the page the frame two back
//! is showing, and frames 0 and 1 come off a cleared screen. The port keeps
//! the two pages in the surface and writes each decoded frame into the page
//! that is off screen at that step; the decoder's own base is that same n-2,
//! measured against the FLI's 42 frames (`sr_assets::anim`'s module docs and
//! `tests/anim.rs`; **A3** and **A4** register the decoder's malformed-input
//! behaviour and two format choices, and this note cites them rather than
//! re-deriving them).
//!
//! # The cadence, and the anchor
//!
//! One tick is one copper frame: `copper2` increments `frame_count` once a
//! retrace (`COPPER.ASM:60-81`), so `while(frame_count<4);` is a four-tick
//! wait and a step draws one animation frame every four ticks. The reference's
//! animation *window* is 215 frames — (7705, 7920] — but that window includes
//! the pre-loop setup and the closing white hold; the step loop itself is the
//! 45 four-tick waits plus whatever time each `ulosta_frame` takes. The port's
//! clock is the fixed four-tick one — see `docs/fidelity/pam.md` §3-§4 for the
//! measurement that makes the cadence reproduce the reference's own keyframes.
//!
//! **The anchor is 7710** — the flat `(251,251,251)` wash that is the
//! reference's first PAM frame (`reference-indices.md` §6.1, three sessions).
//! The source's setup (`:40-45`) comes *after* the gate; the anchored run's
//! gate is open at its start, so that setup is behind it, and the first
//! wait's four frames end at 7712. The counter therefore reads 2 at the
//! anchor (`docs/fidelity/pam.md` §4 derives the 2 from the reference's
//! support frames), which is [`FRAME_COUNT_AT_ANCHOR`].
//!
//! A run whose gate is **closed** at its start is the source's ordinary case:
//! the predecessor's screen stays up through the gate, the setup runs on the
//! tick the gate opens (clearing the screen and zeroing the counter), and the
//! first step is four copper frames later. The anchored run never takes that
//! path; `the_sync_gate_holds_until_code_ten_then_sets_up_and_waits_four_ticks`
//! drives it.
//!
//! # The palette
//!
//! Copy 0 is `PAL.PAL`, and copy 63 is the near-white wash the animation opens
//! and closes on (`wfade[0]`'s 63, used by `tw_setpalette` before the loop).
//! `wfade` is declared `[100]` but has 59 initialisers, and only entries 1-45
//! are ever indexed; the port keeps 59 and the used range 1-45
//! ([`WFADE`]). Copy 63 maps entry 15 to `(63*63 + 8)/64 = 62`, which the DAC
//! widens to 251 — the reference's flat white.
//!
//! # The bugs, preserved
//!
//! - **The FLI header counts 41 frames and the container holds 42**
//!   (`VFLI.C:66` reads the word and never reads it again; `reference-indices.md`
//!   §6, `asset-inventory.md` §E.5). The C's loop draws frames 0-39 — 40 of the
//!   42 — so neither the header's 41 nor the container's 42 is the drawn
//!   count, and the port keeps all three numbers.
//! - **`ruudulle` is dead** (`ASMYT.ASM:207-320`): a second copy of the
//!   unpacker that nothing calls. It is not ported; the source's reader is
//!   `ulosta_frame` alone.
//! - **The `while(frame_count<4);` spin** is a tick-consuming wait, never a
//!   spin, and `exit_requested` is read every waiting tick.
//! - **`wfade`'s 45-step loop outlives its drawing**: steps 41-45 draw
//!   nothing and only put copy 63 back up, which is the closing white hold.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use sr_assets::{load_frames, pal, Format};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

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
/// The `frame_count` the source's counter reads at boot 7710: the first wait
/// began at 7709, so the anchor is its second of four frames.
///
/// Derived in `docs/fidelity/pam.md` §4 from the reference's own support
/// frames: 7759 and 7760 differ while 7761 holds 7760, so the step drawing
/// animation frame 12 starts at 7760; at four ticks a step the first step is
/// 7712, two ticks after the anchor.
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
    ani_path: &'static str,
    pal_path: &'static str,
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
            ani_path: ANI_PATH,
            pal_path: PAL_PATH,
        }
    }

    /// [`new`](Pam::new) reading its assets from elsewhere.
    #[cfg(test)]
    fn at(ani: &'static str, pal: &'static str) -> Pam {
        Pam {
            ani_path: ani,
            pal_path: pal,
            ..Pam::new()
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
        if ctx.music().is_none() {
            return Err(PartError {
                part: NAME,
                message: "this part waits on dis_sync() (PAM/OUTTAA.C:39) and the run has \
                          no music to wait on"
                    .into(),
            });
        }
        let ani = std::fs::read(Path::new(self.ani_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.ani_path),
        })?;
        let frames = load_frames(&ani, Format::Ani).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.ani_path),
        })?;
        if frames.len() != ANI_FRAMES {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} holds {} frames; the container's record chain is {ANI_FRAMES}",
                    self.ani_path,
                    frames.len()
                ),
            });
        }
        for (index, frame) in frames.iter().enumerate() {
            if (frame.width() as usize, frame.height() as usize)
                != (usize::from(VIEW_WIDTH), usize::from(VIEW_HEIGHT))
            {
                return Err(PartError {
                    part: NAME,
                    message: format!(
                        "{} frame {index} is {}x{}; the animation is {VIEW_WIDTH}x{VIEW_HEIGHT}",
                        self.ani_path,
                        frame.width(),
                        frame.height()
                    ),
                });
            }
        }
        self.frames = frames.into_iter().map(|frame| frame.pixels().to_vec()).collect();

        let pal_bytes = std::fs::read(Path::new(self.pal_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.pal_path),
        })?;
        let base = pal::decode(&pal_bytes).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.pal_path),
        })?;
        self.palettes = Pam::washes(&base);

        if ctx.surface.pixels().len() < SURFACE_PIXELS {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "the part's second page reaches pixel {SURFACE_PIXELS} and the surface \
                     holds {}",
                    ctx.surface.pixels().len()
                ),
            });
        }

        self.f = 0;
        // `OUTTAA.C:39-48`: the palette copies are built before the gate
        // (`:36-37`), and the setup is *after* it. An **entry** run whose gate
        // is already open at its start — the anchored run, whose 7710 is two
        // copper frames into the first wait — has that setup behind it, so it
        // runs here with the counter the source reads at the anchor. A part
        // reached mid-chain can also find the gate already open, because the
        // part before it drove the player past the gate's row; then the setup
        // has *not* run, and the source's own `frame_count=0` applies. A run
        // whose gate is still closed keeps the predecessor's screen and
        // performs the setup when the gate opens (also `frame_count=0`).
        let open_at_start = match ctx.music() {
            Some(music) => music.sync() >= SYNC_CODE,
            None => return Err(PartError {
                part: NAME,
                message: "this part waits on dis_sync() (PAM/OUTTAA.C:39) and the run has \
                          no music to wait on"
                    .into(),
            }),
        };
        if open_at_start {
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

    /// `OUTTAA.C:40-45`: the source's post-gate setup — `tw_waitvr` (folded
    /// into the frame count), `tw_setpalette(&pal[768*63])`,
    /// `tw_opengraph`'s clear and mode, `init_uframe` and `init_copper` with
    /// `frame_count` zeroed.
    ///
    /// `frame_count` is the counter the setup leaves: **0** when the gate
    /// opens after the run's start (the source's `:45`), and
    /// [`FRAME_COUNT_AT_ANCHOR`] for the anchored run, whose setup is the two
    /// copper frames before its first tick.
    fn setup(&mut self, ctx: &mut PartCtx, frame_count: u16) {
        // `TWEAK.ASM:27-34`: the mode set clears video memory (the source's
        // `rep stosw` walks 64 KB through all four planes); both pages start
        // black, which is the base frames 0 and 1 are decoded against.
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
        // `COPPER.ASM:37-58`: `copper3` (a bare `retf`) in slots 1 and 0,
        // `copper2` in slot 2. The two no-ops are kept as callbacks because
        // that is what the DIS was handed.
        ctx.set_copper(0, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(1, Box::new(|_ctx: &mut PartCtx| {}));
        ctx.set_copper(2, Box::new(move |ctx: &mut PartCtx| slot2.borrow_mut().slot2(ctx)));
    }

    /// `close_copper` (`COPPER.ASM:14-35`): the DIS's three slots are
    /// unregistered. The port installs `retf`s in their place, which is what
    /// `copper3` already was and leaves nothing that can touch the frame.
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
        // `!dis_exit()` guards both loops (`OUTTAA.C:39`, `:46`); the port reads
        // the flag on every tick, including every tick the wait consumes.
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
            // `OUTTAA.C:40-48`: the source's spin ends and the *setup* runs
            // (`tw_waitvr`, the palette, the mode set, `init_copper`,
            // `frame_count=0`); the loop's first wait then takes its four
            // ticks. The setup tick is not a step.
            self.setup(ctx, 0);
            self.phase = Phase::Loop;
            return PartStatus::Continuing;
        }
        // `OUTTAA.C:48` `while(frame_count<4);`: the wait is a tick-consuming
        // check, and `frame_count` is incremented by copper slot 2 after this
        // call, once a tick.
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
    use crate::shell::{MusicStart, PAM_ANCHOR};
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{sync_from, Music, MusicPosition};
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::view::View;
    use sr_engine::{dac_expand, Surface};

    const TEST_ANI: &str = "../../assets/PAM/OUT.ANI";
    const TEST_PAL: &str = "../../assets/PAM/PAL.PAL";

    /// The music a unit test hands the part: `sync()` answers one scripted
    /// code a tick, every other call is the null reading.
    struct Player {
        codes: Vec<i32>,
        at: usize,
    }

    impl Player {
        fn scripted(codes: Vec<i32>) -> Player {
            Player { codes, at: 0 }
        }

        fn constant(code: i32) -> Player {
            Player { codes: vec![code], at: 0 }
        }
    }

    impl Music for Player {
        fn poll(&mut self) {}
        fn position(&self) -> MusicPosition {
            MusicPosition::default()
        }
        fn musplus(&mut self) -> i32 {
            -32
        }
        fn musrow(&mut self) -> i32 {
            0
        }
        fn muscode(&mut self, _wanted: i32) -> i32 {
            0
        }
        fn sync(&mut self) -> i32 {
            self.codes
                .get(self.at)
                .copied()
                .unwrap_or_else(|| *self.codes.last().unwrap_or(&0))
        }
        fn getmframe(&mut self) -> i32 {
            0
        }
        fn setmframe(&mut self, _frame: i32) {}
    }

    /// The engine's pieces, so a `PartCtx` can be built for one call at a time.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
        player: Player,
        tick: u32,
    }

    impl Rig {
        fn new(player: Player) -> Rig {
            Rig {
                surface: Surface::new(640, 400),
                view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
                palette: Palette::new(),
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
                player,
                tick: 0,
            }
        }

        /// A band's context: never fresh, as the engine's band contexts are.
        fn ctx_at(&mut self, tick: u32, exit: bool) -> PartCtx<'_> {
            self.ctx_with(tick, exit, false)
        }

        /// The init context of an entry-point run — the part the run's music
        /// was installed for.
        fn init_ctx(&mut self, tick: u32) -> PartCtx<'_> {
            self.ctx_with(tick, false, true)
        }

        /// The init context of a part reached mid-chain, with the player
        /// already running.
        fn init_ctx_chain(&mut self, tick: u32) -> PartCtx<'_> {
            self.ctx_with(tick, false, false)
        }

        fn ctx_with(&mut self, tick: u32, exit: bool, music_fresh: bool) -> PartCtx<'_> {
            self.player.at = tick as usize;
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(&mut self.player),
                tick,
                0,
                1,
                true,
                exit,
                music_fresh,
            )
        }

        fn ctx(&mut self, tick: u32) -> PartCtx<'_> {
            self.ctx_at(tick, false)
        }

        fn run_slot(&mut self, slot: usize) {
            let Some(mut callback) = self.copper.take(slot) else {
                return;
            };
            {
                let tick = self.tick;
                let mut ctx = self.ctx(tick);
                callback(&mut ctx);
            }
            self.copper.restore(slot, callback);
        }

        fn step(&mut self, part: &mut Pam) -> PartStatus {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx(tick));
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            status
        }

        fn step_exit(&mut self, part: &mut Pam) -> PartStatus {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx_at(tick, true));
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            status
        }

        fn rendered_rgb(&self) -> Vec<u8> {
            let mut out =
                Vec::with_capacity(usize::from(VIEW_WIDTH) * usize::from(VIEW_HEIGHT) * 3);
            for y in 0..VIEW_HEIGHT {
                for x in 0..VIEW_WIDTH {
                    let at = self.view.source_index(&self.surface, x, y);
                    let entry = self.palette.entry(self.surface.pixels()[at]);
                    out.push(dac_expand(entry[0]));
                    out.push(dac_expand(entry[1]));
                    out.push(dac_expand(entry[2]));
                }
            }
            out
        }

        /// One tick as the engine orders it: slot 0, the part, the compose,
        /// slots 1 and 2. Returns the frame composed during the tick.
        fn step_render(&mut self, part: &mut Pam) -> (PartStatus, Vec<u8>) {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx(tick));
            let frame = self.rendered_rgb();
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            (status, frame)
        }

        fn page(&self, page: usize) -> Vec<u8> {
            let start = PAGE_ORIGIN[page] as usize;
            self.surface.pixels()[start..start + PAGE_PIXELS].to_vec()
        }
    }

    fn started_with(player: Player) -> (Rig, Pam) {
        let mut rig = Rig::new(player);
        let mut part = Pam::at(TEST_ANI, TEST_PAL);
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Pam) {
        started_with(Player::constant(SYNC_CODE))
    }

    fn reference(index: u32) -> Vec<u8> {
        std::fs::read(format!("../../reference/parts/pam/frame_{index:06}.rgb")).unwrap()
    }

    /// **The lengths are asserted first**: a short read must not compare a
    /// prefix and report zero differences as if it had compared the frame.
    fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
        assert_eq!(
            rendered.len(),
            expected.len(),
            "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
            rendered.len(),
            expected.len()
        );
        rendered
            .chunks_exact(3)
            .zip(expected.chunks_exact(3))
            .filter(|(a, b)| a != b)
            .count()
    }

    /// The tick a step lands on: the first step at tick 2 (boot 7712), then
    /// one every four ticks.
    fn step_tick(f: u32) -> u32 {
        FRAME_COUNT_AT_ANCHOR as u32 + 4 * (f - 1)
    }

    /// One 64,000-pixel animation frame through one palette, widened the way
    /// the engine widens it: what the beam composes when the surface holds
    /// `frame` and the live DAC is `palette`.
    fn plain_rgb(frame: &[u8], palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(frame.len() * 3);
        for index in frame {
            let entry = palette.entry(*index);
            out.push(dac_expand(entry[0]));
            out.push(dac_expand(entry[1]));
            out.push(dac_expand(entry[2]));
        }
        out
    }

    // --- the brief's four tests ---------------------------------------------

    #[test]
    fn the_sync_gate_holds_until_code_ten_then_sets_up_and_waits_four_ticks() {
        // `OUTTAA.C:36-48`: the palette copies are built *before* the gate,
        // but the mode set, the near-white palette and `init_copper` happen
        // only after it — so a run whose gate is closed keeps the
        // predecessor's screen, and its counter starts at zero when the gate
        // opens. Code 9 is the table's "before order 13"; code 10 is the
        // block-1 answer from order 13 row 1 (`ordersync1`), the trace's row
        // at boot 7710. The wait consumes ticks, never spins.
        let (mut rig, mut part) = started_with(Player::scripted(vec![9, 9, 9, SYNC_CODE]));
        // What the predecessor left: the part must not touch it during the
        // gate.
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
        assert_eq!(rig.page(0), part.frames[0], "the first step drew frame 0");
    }

    #[test]
    fn a_mid_chain_start_with_the_gate_open_starts_the_counter_at_zero() {
        // Reached mid-chain, an open gate means the part before this one
        // drove the player past the gate's row — not that the run is the
        // anchored entry whose setup already ran. The source's post-gate
        // setup has not run, so its `frame_count=0` is the counter, not the
        // anchor's 2.
        let mut rig = Rig::new(Player::constant(SYNC_CODE));
        let mut part = Pam::at(TEST_ANI, TEST_PAL);
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
        assert_eq!(part_frame_count(), ANI_FRAMES);
        assert_eq!(DRAWN_FRAMES, 40);
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
                    last_page = Some(rig.page(((part.f - 1) & 1) as usize));
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
            let drawn = rig.page(page);
            assert_ne!(drawn, part.frames[40], "frame 40 was drawn");
            assert_ne!(drawn, part.frames[41], "frame 41 was drawn");
        }
        // The last drawn page holds frame 39 (f = 40).
        assert_eq!(last_page.unwrap(), part.frames[39]);
        // `OUTTAA.C:49-61`: f = 41-45 draw nothing and only put the opening
        // wash back; the loop ends on the 45th step.
        assert!(part.finished(), "the loop ended");
    }

    #[test]
    fn the_first_step_uses_wfade_one_half_wash_not_the_opening_sixty_three() {
        // The source writes `[100]` and initialises 59; only 1-45 are indexed.
        assert_eq!(WFADE.len(), 59, "the source's `[100]` has 59 initialisers");
        assert_eq!(WFADE[0], 63, "the pre-loop `tw_setpalette(&pal[768*63])`");
        assert_eq!(WFADE[1], 32, "the first step's copy is the half wash");
        assert_eq!(LOOP_STEPS as usize, 45, "the loop's bound is the used maximum");
        assert_eq!(WFADE[45], 63, "the closing hold's copy");
        let (mut rig, mut part) = started();
        assert_eq!(rig.palette, part.palettes[OPENING_COPY], "the opening white");
        // Ticks 0 and 1 are the wait's last two frames; tick 2 is the step.
        for _ in 0..3 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 1);
        // Slot 2 already uploaded the first step's copy.
        assert_eq!(rig.palette, part.palettes[32], "wfade[1] = 32, not 63");
        // And the opening copy is not the same palette: 32 is a real wash.
        assert_ne!(part.palettes[32], part.palettes[63]);
        // Copy 63 is the near-white one: entry 15 is 62, widened 251.
        assert_eq!(part.palettes[63].entry(15), [62, 62, 62]);
        assert_eq!(dac_expand(62), 251);
        assert_eq!(part.palettes.len(), PALETTE_COPIES);
    }

    #[test]
    fn the_flip_draws_into_the_off_screen_page_and_the_delta_base_is_two_back() {
        // `OUTTAA.C:51-60`: page 0 at step 1, page 1 at step 2, page 0 at
        // step 3; each page holds the frame two back until the new one is
        // unpacked into it.
        let (mut rig, mut part) = started();
        assert_eq!(rig.page(0), vec![0u8; PAGE_PIXELS], "the mode set cleared page 0");
        assert_eq!(rig.page(1), vec![0u8; PAGE_PIXELS], "and page 1");
        // Ticks 0 and 1 wait; tick 2 is step 1 into page 0, displayed.
        for _ in 0..3 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 1);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[0], "the flip shows page 0");
        assert_eq!(rig.page(0), part.frames[0]);
        assert!(rig.page(1).iter().all(|pixel| *pixel == 0), "page 1 is untouched");
        // Ticks 3-5 wait; tick 6 is step 2 into the off-screen page 1.
        for _ in 0..4 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 2);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[1], "the flip shows page 1");
        assert_eq!(rig.page(1), part.frames[1]);
        assert_eq!(rig.page(0), part.frames[0], "page 0 still holds frame 0");
        // Ticks 7-9 wait; tick 10 is step 3 into page 0, which held frame 0.
        for _ in 0..4 {
            rig.step(&mut part);
        }
        assert_eq!(part.f, 3);
        assert_eq!(rig.view.start_addr, PAGE_ORIGIN[0]);
        assert_eq!(rig.page(0), part.frames[2], "frame 2 replaces frame 0");
        assert_ne!(part.frames[2], part.frames[0], "the decode is not a copy of two back");
        assert_eq!(rig.page(1), part.frames[1], "page 1 still holds frame 1");
        // The two pages are the source's two display starts, four pixels a
        // plane byte: 0x4000 plane bytes = 0x10000 pixels.
        assert_eq!(PAGE_ORIGIN[1], 0x1_0000);
    }

    #[test]
    fn exit_requested_while_waiting_ends_the_part_on_that_tick() {
        // `while(frame_count<4);` spins in the source; the port's wait is a
        // tick and the exit flag is read on every one of them.
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
        let (mut rig, mut part) = started_with(Player::constant(9));
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert!(part.finished());
    }

    // --- the record's own numbers -------------------------------------------

    /// The container's frame count and the FLI header's word, from the files:
    /// the off-by-one is the source's and is kept.
    fn part_frame_count() -> usize {
        let ani = std::fs::read(TEST_ANI).unwrap();
        load_frames(&ani, Format::Ani).unwrap().len()
    }

    #[test]
    fn the_fli_header_counts_forty_one_and_the_container_forty_two() {
        // `VFLI.C:66` reads the word and nothing reads it again; the record
        // walk finds 42 (`sr_assets/tests/anim.rs` pins both).
        let fli = std::fs::read("../../assets/PAM/PRAX4.FLI").unwrap();
        assert_eq!(u16::from_le_bytes([fli[6], fli[7]]), 41);
        assert_eq!(part_frame_count(), 42);
        assert_eq!(DRAWN_FRAMES, 40, "and the loop draws neither count");
    }

    // --- the music: the block-1 pre-roll and the trace ----------------------

    /// One frame of a filed trace: the published position at a reference index.
    fn trace_position(name: &str, frame: u32) -> (u8, u8) {
        let path = format!("../../reference/music/{name}/trace.csv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
        for line in text.lines().skip(1) {
            let mut fields = line.split(',');
            let number = |fields: &mut std::str::Split<'_, char>| -> u32 {
                fields.next().unwrap().trim().parse().unwrap()
            };
            let index = number(&mut fields);
            let order = number(&mut fields);
            let row = number(&mut fields);
            if index == frame {
                return (order as u8, row as u8);
            }
        }
        panic!("{name} holds no frame {frame}")
    }

    #[test]
    fn the_block_one_pre_roll_puts_the_player_at_the_traces_7710_position() {
        // `shell::MusicStart::for_part("pam", Some(7710))`: the block-1 clock
        // starts at 400 and the run draws from 7710, so 7310 frames are
        // pre-polled. The engine then polls once for the tick itself, which is
        // the state the part reads.
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

        // The row change is the same frame as the trace's: 7705 (§5 of the
        // note). The port's 7,304th poll is the trace's 7704 row 0.
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

    // --- the keyframes ------------------------------------------------------

    #[test]
    fn the_four_reference_keyframes_are_reproduced() {
        // boot = 7710 + tick. The reference's indices are the anchor's own
        // space, so the ticks are 0, 50, 120 and 190.
        let (mut rig, mut part) = started();
        let wanted: [u32; 13] = [0, 1, 2, 3, 49, 50, 51, 119, 120, 121, 189, 190, 191];
        let mut frames = std::collections::BTreeMap::new();
        while rig.tick <= *wanted.last().unwrap() {
            let tick = rig.tick;
            let (_, frame) = rig.step_render(&mut part);
            if wanted.contains(&tick) {
                frames.insert(tick, frame);
            }
        }
        // The first step's tick composes frame 0 under the *opening* copy 63
        // (white); the next tick shows `wfade[1] = 32`. That is the copper's
        // retrace delay, and it is why a step's palette is visible one tick
        // after its drawing.
        assert_eq!(
            frames[&2],
            plain_rgb(&part.frames[0], &part.palettes[OPENING_COPY]),
            "the first step's tick still shows the opening copy 63"
        );
        assert_eq!(
            frames[&3],
            plain_rgb(&part.frames[0], &part.palettes[usize::from(WFADE[1])]),
            "the next tick shows wfade[1] = 32"
        );
        assert_ne!(frames[&2], frames[&3], "the palette moves one tick later");
        for (tick, index) in [(0u32, 7710u32), (50, 7760), (120, 7830), (190, 7900)] {
            let differing = differing_pixels(&frames[&tick], &reference(index));
            println!(
                "boot {index} (tick {tick}): {differing}/64000 px differ ({:.4} %)",
                100.0 * differing as f64 / 64_000.0
            );
            assert_eq!(differing, 0, "boot {index} (tick {tick}): {differing} pixels differ");
        }
        // The support frames `reference-indices.md` §6.3 measured: 7759 is the
        // frame before and 7761 holds 7760; 7829/7831 hold 7830; 7899/7901
        // hold 7900; 7711 holds 7710.
        assert_ne!(frames[&49], frames[&50], "7759 is a different animation frame");
        assert_eq!(frames[&51], frames[&50], "7761 holds 7760");
        assert_eq!(frames[&119], frames[&120], "7829 holds 7830");
        assert_eq!(frames[&121], frames[&120], "7831 holds 7830");
        assert_eq!(frames[&1], frames[&0], "7711 holds 7710");
        assert_eq!(frames[&189], frames[&190], "7899 holds 7900");
        assert_eq!(frames[&191], frames[&190], "7901 holds 7900");
    }

}
