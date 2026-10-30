//! U2E — the city flythrough, block 3's tail and block 4's entry.
//!
//! `VISU/C/U2E.C` is the vector engine's second part. Its acts:
//!
//! 1. `:185-283` — the mode check (`jellywas`), the packed-scene load, and the
//!    intro palette: the live DAC is captured. The port's runs always start in
//!    its graphics surface, so the BIOS mode is always above 3 and
//!    `jellywas == 1`; a block-4/`--part` entry captures the shell's boot DAC
//!    and a mid-chain entry the predecessor's. The text-mode `vid_init(1)`
//!    initialisation is unreachable here.
//! 2. `:285-335` — the 100-wait intro: 33 fade-in steps, 16 waits, the
//!    `fadeset` border/letterbox into page 0, a CRTC 9 write, the `fadeset` into
//!    pages 1..3, 16 waits, and 33 fade-out steps.
//! 3. `:336-353` — the main setup: `vid_init(11)` (state only), the `U2E.00M`
//!    palette with entries 252..255 patched, the window, and copper slot 2.
//! 4. `:357-366` — the gate: wait for the published order to pass 18.
//! 5. `:376-540` — the loop: the first iteration skips the draw and queues the
//!    fadeset page; thereafter draw, queue and parse at one stream frame per two
//!    retraces.
//! 6. `:541-559` — the teardown: copper removed, the captured DAC faded over 16
//!    retraces from entry 255, then the part ends.
//!
//! The port's tick map, the measured intro frames and the preserved bugs are in
//! `docs/fidelity/u2e.md`.

use std::cell::RefCell;
use std::rc::Rc;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use super::visu::draw::Page;
use super::visu::{
    check_surface, clear_copper, load_scene, parse_frames, view, Clear, Driver, Ring, Scene,
    Variant, PAGES, PAGE_PIXELS, VIEW_WIDTH,
};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "u2e";

/// The packed scene's name (`U2E.C:208`).
const SCENE_NAME: &str = "U2E";

/// The gate (`U2E.C:365`): break when `order > 18`.
const GATE_ORDER: u8 = 18;

/// The intro's step counts (`U2E.C:285-335`).
const FADE_STEPS: u8 = 33;
const HOLD_WAITS: u8 = 16;

/// The teardown's DAC fade (`U2E.C:543-555`): 16 steps of +4, clamp 63.
const TEARDOWN_STEPS: u8 = 16;

/// A planar byte address in the original's 320x200 layout is four linear pixels
/// (`docs/fidelity/pam.md` §2); the source's `memset`s walk addresses, and the
/// fadeset's `+252` stray lands 1,008 pixels into a later row.
const PLANE_BYTES_PER_ROW: usize = 80;

/// Where the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// A fade-in step: `fpal` mutated, then (next tick) uploaded.
    FadeIn { step: u8 },
    /// The 16 waits after the fade-in (`U2E.C:297-300`).
    FadeInHold { left: u8 },
    /// `fadeset` into page 0 (`U2E.C:303`), at part tick 49.
    FadeSetFirst,
    /// CRTC 9 `(old & 0xf0) | 0x80` (`U2E.C:305-308`): state only.
    Crtc,
    /// `fadeset` into pages 1..3 (`U2E.C:310-312`), then the first of the 16 waits.
    FadeSetRest,
    /// The 15 remaining waits after `fadeset`'s second application.
    FadeRestHold { left: u8 },
    /// A fade-out step (`U2E.C:320-335`).
    FadeOut { step: u8 },
    /// `vid_init(11)`, the main palette, the window, the copper and the ring.
    Setup,
    /// The music gate (`U2E.C:357-366`).
    Gate,
    /// The draw loop (`U2E.C:376-540`).
    Loop,
    /// The DAC fade (`U2E.C:541-555`).
    Teardown { left: u8 },
    /// The part has ended.
    Done,
}

/// U2E: the scene, the two palette states and the shared driver.
pub struct U2e {
    /// The loaded `U2E.00M`/`.0AA`/`.0AB` scene.
    scene: Option<Scene>,
    /// `U2E.00M`'s palette with entries 252..255 patched (`U2E.C:337-350`).
    main_palette: Palette,
    /// The ring and projection, shared with copper slot 2.
    driver: Option<Rc<RefCell<Driver>>>,
    /// `fpal[768]`: the intro's and the teardown's working DAC.
    fpal: [u8; 768],
    /// Whether `fpal` has an intro mutation waiting for the next tick's upload.
    /// The source mutates, waits and then uploads, so a fade's step shows one
    /// tick after it is computed; this flag is that one-tick delay.
    palette_dirty: bool,
    /// The same one-tick delay for the teardown, whose upload is the rotated
    /// DAC write rather than the intro's straight 768-byte copy.
    teardown_dirty: bool,
    phase: Phase,
    /// CRTC register 9's scan-doubling bit (`U2E.C:305-308`): display hardware
    /// state only, kept for the record and the tests.
    crtc_scan_doubled: bool,
}

impl U2e {
    /// A part with nothing loaded: `init` reads the scene.
    pub fn new() -> U2e {
        U2e {
            scene: None,
            main_palette: Palette::new(),
            driver: None,
            fpal: [0; 768],
            palette_dirty: false,
            teardown_dirty: false,
            phase: Phase::FadeIn { step: 0 },
            crtc_scan_doubled: false,
        }
    }

    /// Read the scene, choose the intro palette and set the view.
    ///
    /// The source's `jellywas` is the BIOS video mode at `:198-205`: `a > 3`
    /// is mode 13h and above, and `jellywas == 1` captures the live DAC
    /// (`:281-282`) instead of running `vid_init(1)`'s text-mode palette
    /// initialisation (`:274-277`).
    ///
    /// **The port's runs always take the capture.** Every run shape begins in
    /// the shell's graphics surface — `--block 4` and `--part u2e` from the
    /// boot DAC `hardware_default_palette` installs, a chain from whatever its
    /// predecessor left — so the mode the source reads is always above 3. The
    /// `vid_init(1)` branch is unreachable and is not implemented; the
    /// substitution changes no measured intro frame, because the fade-in
    /// saturates every byte it touches either way. Recorded in
    /// `docs/fidelity/u2e.md` §4.
    fn prepare(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part waits on the music's published order (U2E.C:357-366) and the run \
             has no music to wait on",
        )?;

        check_surface(ctx, NAME)?;
        let mut scene = load_scene(SCENE_NAME).map_err(|message| PartError::new(NAME, message))?;
        scene.reset();

        let mut main_palette = scene.master.palette;
        main_palette.set_entry(252, [0, 0, 0]);
        main_palette.set_entry(253, [63, 63, 63]);
        main_palette.set_entry(254, [63, 63, 63]);
        main_palette.set_entry(255, [0, 0, 0]);

        // `outp(0x3c7,0); for(a=0;a<768;a++) fpal[a]=inp(0x3c9);`
        // (`U2E.C:281-282`): the live DAC, six bits a component. Every run
        // shape reaches this branch — see the doc comment above — including a
        // block-4 entry, which captures the shell's boot DAC.
        self.fpal = ctx.palette.to_six_bit();
        self.palette_dirty = false;
        self.teardown_dirty = false;

        // `vid_init(1)`/`vid_init(11)` are state only for the port: both modes
        // are the shipped 320x200 tweak (`AVIDM1.ASM:17-42`, `:44-69`), and
        // the window is `Driver::new`'s `vid_window(0,319,25,174,512,
        // 9999999)` (`U2E.C:352`). Page 0 is the intro's picture; the display
        // start stays 0 until the ring's copper moves it.
        *ctx.view = view();

        self.scene = Some(scene);
        self.main_palette = main_palette;
        self.driver = None;
        self.crtc_scan_doubled = false;
        self.phase = Phase::FadeIn { step: 0 };
        Ok(())
    }
}

impl U2e {
    /// Upload a fade mutation that the previous tick computed: the source's
    /// `dis_waitb(); outp(0x3c8,0); outp(0x3c9,...)` (`U2E.C:292-295`), whose
    /// upload is one retrace after the mutation.
    fn flush_palette(&mut self, ctx: &mut PartCtx) {
        if self.palette_dirty {
            ctx.set_palette(Palette::from_six_bit(&self.fpal));
            self.palette_dirty = false;
        }
    }

    /// Upload a teardown mutation one tick after it was computed: the same
    /// `dis_waitb(); outp(...)` delay as the intro's, with the rotated write
    /// (`U2E.C:552-554`).
    fn flush_teardown(&mut self, ctx: &mut PartCtx) {
        if self.teardown_dirty {
            ctx.set_palette(rotated_palette(&self.fpal));
            self.teardown_dirty = false;
        }
    }

    /// `for(a=3;a<768-6;a++) { fpal[a]+=2; if(fpal[a]>63) fpal[a]=63; }`
    /// (`U2E.C:287-291`).
    fn fade_in_step(&mut self) {
        for byte in &mut self.fpal[3..768 - 6] {
            *byte = (*byte + 2).min(63);
        }
        self.palette_dirty = true;
    }

    /// `U2E.C:322-331`: entries 1..252 fade down, 253..254 fade up, entry 255
    /// and entry 0 are untouched.
    fn fade_out_step(&mut self) {
        for byte in &mut self.fpal[3..768 - 9] {
            *byte = byte.saturating_sub(2);
        }
        for byte in &mut self.fpal[768 - 9..768 - 3] {
            *byte = (*byte + 2).min(63);
        }
        self.palette_dirty = true;
    }

    /// `dis_setcopper(2,copper2)` (`U2E.C:353`): the ring's copper, shared
    /// with the part through the `Rc`.
    fn install_copper(&mut self, ctx: &mut PartCtx) {
        let driver = Rc::new(RefCell::new(Driver::new(Ring::new(Variant::U2e), true)));
        let slot2 = Rc::clone(&driver);
        ctx.set_copper(
            2,
            Box::new(move |ctx: &mut PartCtx| {
                let mut driver = slot2.borrow_mut();
                // `copper2` (`U2E.C:115-143`), then `vid_setswitch(-1,clr)`:
                // the display start is the new page's 64,000-pixel origin.
                driver.ring.isr();
                let origin = driver.ring.displayed() as u32 * PAGE_PIXELS as u32;
                ctx.view.set_origin(origin);
            }),
        );
        self.driver = Some(driver);
    }

    /// `U2E.C:336-353`: `vid_init(11)` is state only, the palette is the
    /// patched master, the window is `Driver::new`'s, and the copper and ring
    /// start here — before the gate, so the gate's retraces drain the ring's
    /// 16-retrace start-up delay.
    fn setup(&mut self, ctx: &mut PartCtx) {
        ctx.set_palette(self.main_palette);
        self.install_copper(ctx);
        self.phase = Phase::Gate;
        self.gate_tick(ctx);
    }

    /// One tick of `while(!dis_exit()) { poll; if(a>18) break; }`
    /// (`U2E.C:357-366`).
    fn gate_tick(&mut self, ctx: &mut PartCtx) {
        // The source's `while(!dis_exit())` tests the exit first; falling out
        // of it runs the ring setup and then the main loop's own exit test,
        // which tears down (`U2E.C:376`, `:541`).
        if ctx.exit_requested {
            self.teardown(ctx);
            return;
        }
        let open = ctx
            .music()
            .is_some_and(|music| music.position().order > GATE_ORDER);
        if open {
            self.phase = Phase::Loop;
            self.loop_tick(ctx);
        }
    }

    /// One iteration of `U2E.C:376-540`.
    fn loop_tick(&mut self, ctx: &mut PartCtx) {
        // `while(!dis_exit() && !xit)` (`U2E.C:376`).
        if ctx.exit_requested {
            self.teardown(ctx);
            return;
        }
        let Some(driver) = self.driver.as_ref().map(Rc::clone) else {
            return;
        };

        let parse = {
            let mut driver = driver.borrow_mut();
            let scene = self
                .scene
                .as_mut()
                .expect("the scene is loaded before the loop");
            if driver.first_draw {
                // `} else { syncframe=0; firstframe=0; coppercnt=1; }`
                // (`U2E.C:433`): no page is taken, nothing is cleared and
                // nothing is drawn; the fadeset page (the writer page, 0) is
                // scheduled and queued below. The driver's `first_draw` branch
                // performs the clock reset.
                let mut empty: [u8; 0] = [];
                let mut page = Page::new(&mut empty, usize::from(VIEW_WIDTH));
                driver.draw_frame(scene, &mut page, Clear::Byte(0));
                let (frames, parse) = driver.ring.schedule();
                driver.ring.queue(0, frames);
                parse
            } else {
                // `while(cl[clw].ready) { if(deadlock>16) break; }`
                // (`U2E.C:389-392`): a busy page is this tick's wait.
                let Some(page) = driver.ring.next_page() else {
                    return;
                };
                let start = page * PAGE_PIXELS;
                {
                    let mut target = Page::new(
                        &mut ctx.surface.pixels_mut()[start..start + PAGE_PIXELS],
                        usize::from(VIEW_WIDTH),
                    );
                    // `vid_setswitch(clw,-1); vid_clear255(); vid_cameraangle;
                    // order and draw` (`U2E.C:394-431`).
                    driver.draw_frame(scene, &mut target, Clear::Byte(255));
                }
                // `cl[clw].frames=repeat; ready=1; clw++; repeat=(repeat+1)/2;
                // currframe+=repeat*2` (`U2E.C:434-444`).
                let (frames, parse) = driver.ring.schedule();
                driver.ring.queue(page, frames);
                parse
            }
        };

        let scene = self.scene.as_mut().expect("the scene is loaded before the loop");
        if parse_frames(scene, &driver, parse) {
            self.teardown(ctx);
        }
    }

    /// `U2E.C:541-555`: slot 2 removed, the live DAC captured into `fpal`, and
    /// the first of the sixteen fade steps computed on this tick. The first
    /// upload is the next tick's, as `dis_waitb()` precedes it in the source.
    fn teardown(&mut self, ctx: &mut PartCtx) {
        clear_copper(ctx);
        self.fpal = ctx.palette.to_six_bit();
        self.palette_dirty = false;
        self.teardown_dirty = false;
        self.phase = Phase::Teardown {
            left: TEARDOWN_STEPS - 1,
        };
        self.teardown_mutate();
    }

    /// One `fpal[a]+=4` pass (`U2E.C:545-550`); the 768-byte DAC write from
    /// entry 255 follows on the next tick (`:552-554`). The write wraps: entry
    /// 255 takes `fpal`'s entry 0 and entry `j < 255` takes `fpal`'s entry
    /// `j + 1`.
    fn teardown_mutate(&mut self) {
        for byte in &mut self.fpal {
            *byte = (*byte + 4).min(63);
        }
        self.teardown_dirty = true;
    }

    fn status(&self) -> PartStatus {
        if self.phase == Phase::Done {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}

impl Part for U2e {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        self.prepare(ctx)
    }

    /// U2E is the one part whose source runs a **timed teardown after
    /// `dis_exit`**: `U2E.C:541-555` removes the copper, captures the DAC and
    /// fades it over sixteen retraces, whichever way the gate or loop exits.
    /// Retiring the part on the exit tick would cut that fade to nothing, so
    /// the shell keeps it live until it reports `Finished` itself.
    fn drains_after_exit(&self) -> bool {
        true
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.flush_palette(ctx);
        self.flush_teardown(ctx);
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::FadeIn { step } => {
                self.fade_in_step();
                self.phase = if step + 1 >= FADE_STEPS {
                    Phase::FadeInHold { left: HOLD_WAITS }
                } else {
                    Phase::FadeIn { step: step + 1 }
                };
                PartStatus::Continuing
            }
            Phase::FadeInHold { left } => {
                self.phase = if left <= 1 {
                    Phase::FadeSetFirst
                } else {
                    Phase::FadeInHold { left: left - 1 }
                };
                PartStatus::Continuing
            }
            Phase::FadeSetFirst => {
                fadeset(ctx.surface.pixels_mut(), 0);
                self.phase = Phase::Crtc;
                PartStatus::Continuing
            }
            Phase::Crtc => {
                self.crtc_scan_doubled = true;
                self.phase = Phase::FadeSetRest;
                PartStatus::Continuing
            }
            Phase::FadeSetRest => {
                let pixels = ctx.surface.pixels_mut();
                for page in 1..PAGES {
                    fadeset(pixels, page);
                }
                self.phase = Phase::FadeRestHold {
                    left: HOLD_WAITS - 1,
                };
                PartStatus::Continuing
            }
            Phase::FadeRestHold { left } => {
                self.phase = if left <= 1 {
                    Phase::FadeOut { step: 1 }
                } else {
                    Phase::FadeRestHold { left: left - 1 }
                };
                PartStatus::Continuing
            }
            Phase::FadeOut { step } => {
                self.fade_out_step();
                self.phase = if step >= FADE_STEPS {
                    Phase::Setup
                } else {
                    Phase::FadeOut { step: step + 1 }
                };
                PartStatus::Continuing
            }
            Phase::Setup => {
                self.setup(ctx);
                self.status()
            }
            Phase::Gate => {
                self.gate_tick(ctx);
                self.status()
            }
            Phase::Loop => {
                self.loop_tick(ctx);
                self.status()
            }
            Phase::Teardown { left } => {
                // The previous tick's mutation was flushed above; when all
                // sixteen are computed and uploaded, the part ends on the tick
                // that shows the last one.
                if left == 0 {
                    self.phase = Phase::Done;
                    PartStatus::Finished
                } else {
                    self.teardown_mutate();
                    self.phase = Phase::Teardown { left: left - 1 };
                    PartStatus::Continuing
                }
            }
        }
    }
}

/// `fadeset` (`U2E.C:145-183`) into one packed page.
///
/// The source writes VGA plane bytes: `outp(0x3c5,15)` makes a `memset` address
/// write all four planes, and the `3c5=2+4+8` write at byte 63 sets only planes
/// 1..3. The port expands every address to `4*A+p` in a linear 320x200 page.
/// The top block's last `memset` is at `vram+y*80+252` — 172 addresses past the
/// row it just wrote, which lands in rows `y+3..`; every one of those addresses
/// is written again by a later row pass, so the stray has no visible effect.
/// Every write the shipped pattern makes is inside its page (the largest is the
/// bottom row's byte 15,999), so the `pixel < PAGE_PIXELS` guard never fires
/// for it; it is a bound against a malformed pattern, and the unit test's
/// sentinel proves a page's writes never cross into the next page.
fn fadeset(pixels: &mut [u8], page: usize) {
    let start = page * PAGE_PIXELS;
    let page = &mut pixels[start..start + PAGE_PIXELS];
    for y in 0..25 {
        planar_fill(page, y * PLANE_BYTES_PER_ROW, 17, 0x0F, 0);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 17, 47, 0x0F, 252);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 63, 1, 0x0E, 0);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 252, 16, 0x0F, 0);
    }
    for y in 25..175 {
        planar_fill(page, y * PLANE_BYTES_PER_ROW, 17, 0x0F, 254);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 17, 47, 0x0F, 253);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 63, 1, 0x0E, 254);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 64, 16, 0x0F, 254);
    }
    for y in 175..200 {
        planar_fill(page, y * PLANE_BYTES_PER_ROW, 17, 0x0F, 0);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 17, 47, 0x0F, 252);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 63, 1, 0x0E, 0);
        planar_fill(page, y * PLANE_BYTES_PER_ROW + 64, 16, 0x0F, 0);
    }
}

/// `len` plane-byte writes at `address..address+len`, each setting the linear
/// pixels `4*A+p` for every plane `p` in `mask`.
fn planar_fill(page: &mut [u8], address: usize, len: usize, mask: u8, value: u8) {
    for at in address..address + len {
        for plane in 0..4 {
            if mask & (1 << plane) != 0 {
                let pixel = at * 4 + plane;
                if pixel < PAGE_PIXELS {
                    page[pixel] = value;
                }
            }
        }
    }
}

/// The DAC as `outp(0x3c8,255)` followed by 768 byte writes leaves it
/// (`U2E.C:553-554`): the write index wraps from 255 to 0, so entry 255 takes
/// `fpal`'s entry 0 and entry `j < 255` takes `fpal`'s entry `j + 1`. The last
/// three bytes of `fpal` land in entry 254 and none are left unwritten.
fn rotated_palette(fpal: &[u8; 768]) -> Palette {
    let mut palette = Palette::new();
    palette.set_entry(255, [fpal[0], fpal[1], fpal[2]]);
    for index in 0..255u8 {
        let at = 3 * (usize::from(index) + 1);
        palette.set_entry(index, [fpal[at], fpal[at + 1], fpal[at + 2]]);
    }
    palette
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::visu::VIEW_HEIGHT;
    use sr_engine::copper::COPPER_SLOTS;
    use crate::parts::test_rig::{differing_pixels, reference, rgb_of, Rig, ScriptedMusic};
    use sr_engine::dac_expand;

    fn constant(order: u8, row: u8) -> ScriptedMusic {
        ScriptedMusic::positions(vec![(order, row)])
    }

    /// The gate's closed position (`second4`'s order 18).
    fn closed() -> ScriptedMusic {
        constant(GATE_ORDER, 0)
    }

    /// Closed through tick `open - 1`, order 19 from `open` on. Twenty
    /// ticks of gate drain the ring's 16-retrace start-up delay, as the
    /// shipped gate's 181 ticks do.
    fn opening_at(open: usize) -> ScriptedMusic {
        let mut positions = vec![(GATE_ORDER, 0); open];
        positions.push((GATE_ORDER + 1, 0));
        ScriptedMusic::positions(positions)
    }

    /// The part tick the loop's tests open their gate at: twenty ticks after
    /// the setup, enough to drain the ring's start-up delay.
    const OPEN: usize = 120;

    fn rig(music: ScriptedMusic) -> Rig {
        Rig::new(VIEW_WIDTH, VIEW_HEIGHT).with(music)
    }

    fn page_of(rig: &Rig, page: usize) -> &[u8] {
        let start = page * PAGE_PIXELS;
        &rig.surface.pixels()[start..start + PAGE_PIXELS]
    }

    fn started_with(music: ScriptedMusic) -> (Rig, U2e) {
        let mut rig = rig(music);
        let mut part = U2e::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    /// Drive the part to the start of the gate. The intro ends on tick 100: the
    /// fade-out's last step and the main setup share that tick.
    fn to_gate(rig: &mut Rig, part: &mut U2e) {
        for _ in 0..=100 {
            rig.step(part);
        }
        assert_eq!(part.phase, Phase::Gate, "the intro is 101 ticks");
    }

    /// Drive the part through the intro and the gate, leaving the next call to
    /// be the loop's first tick at part tick [`OPEN`].
    fn to_loop(rig: &mut Rig, part: &mut U2e) {
        for _ in 0..OPEN {
            rig.step(part);
        }
        assert_eq!(rig.tick, OPEN as u32);
        assert_eq!(part.phase, Phase::Gate);
    }

    // --- the intro ----------------------------------------------------------

    #[test]
    fn a_jelly_boot_captures_the_live_dac() {
        let mut rig = rig(closed());
        let mut live = Palette::new();
        live.set_entry(0, [1, 2, 3]);
        live.set_entry(7, [5, 6, 7]);
        live.set_entry(255, [63, 1, 2]);
        rig.palette = live;
        let mut part = U2e::new();
        part.init(&mut rig.ctx(0)).unwrap();
        assert_eq!(part.fpal, live.to_six_bit());
        assert_eq!(&part.fpal[0..3], &[1, 2, 3]);
        assert_eq!(&part.fpal[3 * 7..3 * 7 + 3], &[5, 6, 7]);
        assert_eq!(&part.fpal[3 * 255..], &[63, 1, 2]);
        assert_eq!(rig.palette, live, "the capture reads, it does not write");
        assert_eq!(part.phase, Phase::FadeIn { step: 0 });
    }

    #[test]
    fn a_block_four_shaped_init_captures_the_live_dac_too() {
        // **The entry shape**, where the old `music_fresh` proxy used to take
        // `vid_init(1)`'s text-mode palette. The port's block-4 run starts in
        // its graphics surface with the shell's boot DAC installed
        // (`hardware_default_palette`, the emulator's mode-set table), so the
        // BIOS mode is above 3 here exactly as it is mid-chain and the source
        // takes the capture branch (`U2E.C:281-282`). The palette used is a
        // non-black one, so a capture is visible against a fresh `fpal`.
        let mut rig = rig(closed());
        let mut live = Palette::new();
        live.set_entry(0, [63, 0, 0]);
        live.set_entry(7, [5, 6, 7]);
        live.set_entry(247, [11, 12, 16]);
        rig.palette = live;
        let mut part = U2e::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        assert_eq!(
            part.fpal,
            live.to_six_bit(),
            "the block-4 shape captures the shell's DAC, not vid_init(1)'s"
        );
        assert_eq!(&part.fpal[3 * 7..3 * 7 + 3], &[5, 6, 7]);
        assert_eq!(&part.fpal[3 * 247..3 * 247 + 3], &[11, 12, 16]);
        assert_eq!(rig.palette, live, "init uploads no palette (U2E.C:273-283)");
        assert_eq!(part.phase, Phase::FadeIn { step: 0 });
    }

    #[test]
    fn the_fade_in_runs_33_steps_and_saturates_entries_1_to_253() {
        let (mut rig, mut part) = started_with(closed());
        // One mutation a tick, uploaded one tick later: after 33 mutations
        // fpal is saturated and the 33rd upload is the 34th tick's palette.
        for _ in 0..FADE_STEPS {
            rig.step(&mut part);
        }
        assert_eq!(part.phase, Phase::FadeInHold { left: HOLD_WAITS });
        for index in 0..=255u8 {
            let expected = if (1..=253).contains(&index) { 63 } else { 0 };
            assert_eq!(part.fpal[3 * index as usize], expected, "entry {index}");
            assert_eq!(part.fpal[3 * index as usize + 1], expected, "entry {index} g");
            assert_eq!(part.fpal[3 * index as usize + 2], expected, "entry {index} b");
        }
        // The 33rd upload lands on the first hold tick.
        assert_eq!(rig.palette.entry(1), [63, 63, 63]);
        assert_eq!(rig.palette.entry(253), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [0, 0, 0]);
        assert_eq!(rig.palette.entry(0), [0, 0, 0]);
    }

    #[test]
    fn the_fadeset_lands_on_tick_49_and_paints_the_measured_band() {
        let (mut rig, mut part) = started_with(closed());
        for tick in 0..49 {
            let (_, frame) = rig.step_render(&mut part);
            assert!(
                frame.iter().all(|&byte| byte == 0),
                "tick {tick}: the loader's black screen holds"
            );
        }
        assert_eq!(part.phase, Phase::FadeSetFirst, "the holds are spent");
        // Tick 49 is the fadeset: 185 white at x 68..252 and 135 black in
        // every row (the dense scan in `docs/fidelity/u2e.md`).
        let (_, frame) = rig.step_render(&mut part);
        assert_eq!(part.phase, Phase::Crtc);
        let mut white = 0;
        for y in 0..usize::from(VIEW_HEIGHT) {
            for x in 0..usize::from(VIEW_WIDTH) {
                let at = (y * usize::from(VIEW_WIDTH) + x) * 3;
                let pixel = &frame[at..at + 3];
                let expected_white = (68..=252).contains(&x);
                if expected_white {
                    white += 1;
                    assert_eq!(pixel, [255, 255, 255], "row {y} x {x}");
                } else {
                    assert_eq!(pixel, [0, 0, 0], "row {y} x {x}");
                }
            }
        }
        assert_eq!(white, 185 * 200);
    }

    #[test]
    fn the_fadeset_writes_the_source_s_pattern_within_its_page() {
        // The source's top block writes bytes 0..63 (x 0..255) and then a
        // `memset(vram+y*80+252,0,16)` that lands in a later row; the middle
        // and bottom blocks write x 0..319. A sentinel shows the columns the
        // top block never writes and proves page 0's writes stay in page 0
        // (the shipped pattern is entirely in-page; the guard is a bound).
        let mut pixels = vec![0xEEu8; 2 * PAGE_PIXELS];
        {
            let (page0, rest) = pixels.split_at_mut(PAGE_PIXELS);
            fadeset(page0, 0);
            assert!(
                rest.iter().all(|&pixel| pixel == 0xEE),
                "the fadeset never writes across the page boundary"
            );
        }
        let row = |y: usize, x: usize| pixels[y * 320 + x];
        // The top block.
        assert_eq!(row(0, 0), 0);
        assert_eq!(row(0, 67), 0);
        assert_eq!(row(0, 68), 252);
        assert_eq!(row(0, 252), 252, "byte 63 keeps plane 0");
        assert_eq!(row(0, 253), 0);
        assert_eq!(row(0, 255), 0);
        assert_eq!(row(0, 256), 0xEE, "top rows never write x 256..319");
        assert_eq!(row(24, 319), 0xEE);
        // The middle block.
        assert_eq!(row(25, 0), 254);
        assert_eq!(row(25, 67), 254);
        assert_eq!(row(25, 68), 253);
        assert_eq!(row(25, 252), 253);
        assert_eq!(row(25, 253), 254);
        assert_eq!(row(25, 319), 254);
        // The bottom block.
        assert_eq!(row(175, 0), 0);
        assert_eq!(row(175, 68), 252);
        assert_eq!(row(175, 253), 0);
        assert_eq!(row(175, 319), 0);
        // The stray `+252` write is overwritten by the rows it lands in; the
        // only un-overwritten part is the boundary test above.
        assert_eq!(row(199, 80), 252);
    }

    #[test]
    fn the_fadeset_reaches_pages_1_to_3_at_tick_51() {
        let (mut rig, mut part) = started_with(closed());
        for _ in 0..51 {
            rig.step(&mut part);
        }
        assert_eq!(part.phase, Phase::FadeSetRest);
        // The CRTC 9 write on tick 50 is state only: the port's view is the
        // same 320x200 geometry (the ruling's display-hardware detail).
        assert!(part.crtc_scan_doubled);
        assert_eq!((rig.view.width, rig.view.height), (VIEW_WIDTH, VIEW_HEIGHT));
        let (_, _) = rig.step_render(&mut part);
        assert_eq!(part.phase, Phase::FadeRestHold { left: 15 });
        for page in 1..PAGES {
            assert_eq!(page_of(&rig, page)[0], 0, "page {page} fadeset");
            assert_eq!(page_of(&rig, page)[25 * 320 + 68], 253, "page {page}");
            assert_eq!(page_of(&rig, page)[175 * 320 + 68], 252, "page {page}");
        }
        assert_eq!(page_of(&rig, 0)[25 * 320 + 68], 253, "page 0 keeps its copy");
    }

    #[test]
    fn the_fade_out_starts_on_tick_68_and_matches_the_measured_490_and_500() {
        let (mut rig, mut part) = started_with(closed());
        for _ in 0..67 {
            rig.step(&mut part);
        }
        assert_eq!(part.phase, Phase::FadeOut { step: 1 });
        assert_eq!(part.fpal[3 * 252], 63, "no mutation before tick 67");
        // Tick 67 computes step 1; the upload is the next tick's, so the live
        // DAC at 67 still holds the saturated fade-in.
        rig.step(&mut part);
        assert_eq!(part.phase, Phase::FadeOut { step: 2 });
        assert_eq!(rig.palette.entry(252), [63, 63, 63]);
        assert_eq!(part.fpal[3 * 252], 61);
        rig.step(&mut part);
        assert_eq!(rig.palette.entry(252), [61, 61, 61], "step 1 at tick 68");

        // Boot 490 is part tick 72 under the derived 418 anchor: five steps.
        while rig.tick < 72 {
            rig.step(&mut part);
        }
        let (_, frame) = rig.step_render(&mut part);
        // The capture out/u2e-start/frame_000490.rgb: 215 in the top/bottom
        // band (entry 252 = 53) and 40 at the middle block's sides (entry
        // 254 = 10).
        assert_eq!(&frame[68 * 3..68 * 3 + 3], &[215, 215, 215], "top band");
        assert_eq!(&frame[(100 * 320) * 3..(100 * 320) * 3 + 3], &[40, 40, 40]);
        assert_eq!(
            &frame[(100 * 320 + 68) * 3..(100 * 320 + 68) * 3 + 3],
            &[255, 255, 255],
            "entry 253 stays white"
        );

        // Boot 500 is part tick 82: fifteen steps.
        while rig.tick < 82 {
            rig.step(&mut part);
        }
        let (_, frame) = rig.step_render(&mut part);
        assert_eq!(&frame[68 * 3..68 * 3 + 3], &[134, 134, 134], "top band");
        assert_eq!(&frame[(100 * 320) * 3..(100 * 320) * 3 + 3], &[121, 121, 121]);
    }

    #[test]
    fn the_main_palette_goes_up_at_tick_100_and_holds_the_letterbox() {
        let (mut rig, mut part) = started_with(closed());
        for _ in 0..=100 {
            rig.step(&mut part);
        }
        assert_eq!(part.phase, Phase::Gate, "the setup shares the last fade tick");
        assert_eq!(rig.palette, part.main_palette, "U2E.00M with 252..255 patched");
        assert_eq!(part.main_palette.entry(252), [0, 0, 0]);
        assert_eq!(part.main_palette.entry(253), [63, 63, 63]);
        assert_eq!(part.main_palette.entry(254), [63, 63, 63]);
        assert_eq!(part.main_palette.entry(255), [0, 0, 0]);
        let scene = part.scene.as_ref().unwrap();
        assert_eq!(part.main_palette.entry(0), scene.master.palette.entry(0));
        assert_eq!(part.main_palette.entry(251), scene.master.palette.entry(251));

        // The hold: rows 25..174 are white, the rest black; 48,000 white at
        // dac_expand(63) and 16,000 black, mean 191.25 (`reference-indices.md`
        // §18.6's befaf38a... hold).
        let (_, frame) = rig.step_render(&mut part);
        let mut sum = 0u64;
        for y in 0..200usize {
            for x in 0..320usize {
                let at = (y * 320 + x) * 3;
                let white = (25..175).contains(&y);
                if white {
                    assert_eq!(&frame[at..at + 3], &[255, 255, 255], "row {y}");
                    sum += 255;
                } else {
                    assert_eq!(&frame[at..at + 3], &[0, 0, 0], "row {y}");
                }
            }
        }
        assert_eq!(sum as f64 / 64_000.0, 191.25);
    }

    // --- the view -----------------------------------------------------------

    #[test]
    fn the_view_is_the_shipped_320x200_from_the_intro_on() {
        let (rig, part) = started_with(closed());
        assert_eq!((rig.view.width, rig.view.height), (VIEW_WIDTH, VIEW_HEIGHT));
        assert_eq!(rig.view.start_addr, 0);
        assert_eq!(rig.view.y_line_offset, Some(320), "CRTC 0x13: 320 pixels a row");
        assert!(!rig.view.double_scan);
        assert!(part.driver.is_none(), "the ring arrives with the setup");
        assert!(!rig.copper.is_set(2), "and so does its copper");
    }

    // --- the gate and the loop ---------------------------------------------

    #[test]
    fn the_gate_polls_once_a_tick_until_order_19_and_exit_tears_down() {
        // The setup polls at tick 100; ticks 0..100 are closed, and tick 101
        // is the first order 19 poll.
        let (mut rig, mut part) = started_with(opening_at(101));
        for tick in 0..=100 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing, "tick {tick}");
        }
        assert_eq!(part.phase, Phase::Gate);
        assert!(part.driver.is_some(), "the copper runs during the gate");
        assert!(rig.copper.is_set(2));
        // Tick 101 is the first order 19 poll: the loop starts on it.
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Loop);
        let scene = part.scene.as_ref().unwrap();
        assert_eq!(scene.stream, 1, "the first loop tick parsed one stream frame");

        // Exit during the gate: the source's `while(!dis_exit())` falls through
        // to the ring setup and the loop's own exit test, which tears down.
        let (mut rig, mut part) = started_with(closed());
        to_gate(&mut rig, &mut part);
        assert_eq!(part.driver.as_ref().unwrap().borrow().ring.displayed(), 0);
        let before = rig.surface.pixels().to_vec();
        let status = rig.step_exit(&mut part);
        assert_eq!(status, PartStatus::Continuing);
        assert!(matches!(part.phase, Phase::Teardown { .. }));
        assert_eq!(rig.surface.pixels(), before, "no page is cleared");
        for slot in 0..COPPER_SLOTS {
            assert!(rig.copper.is_set(slot), "slot {slot} holds a no-op");
        }
    }

    #[test]
    fn the_first_loop_tick_draws_nothing_and_queues_the_fadeset_page() {
        let (mut rig, mut part) = started_with(opening_at(OPEN));
        to_loop(&mut rig, &mut part);
        let fadeset_page = page_of(&rig, 0).to_vec();
        let origin = rig.view.start_addr;
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Loop);
        assert_eq!(page_of(&rig, 0), fadeset_page, "no draw on the first loop tick");
        assert_eq!(rig.view.start_addr, origin, "page 0 stays displayed");
        assert!(!part.driver.as_ref().unwrap().borrow().first_draw);
        assert_eq!(part.scene.as_ref().unwrap().stream, 1);
        assert_eq!(part.driver.as_ref().unwrap().borrow().fov, 0x1c00);
    }

    #[test]
    fn the_cadence_over_eight_ticks_advances_four_stream_frames() {
        let (mut rig, mut part) = started_with(opening_at(OPEN));
        to_loop(&mut rig, &mut part);
        // The first loop tick parses frame 0 and queues page 0; the steady
        // state is one stream frame per two ticks (U2E.C:434-444).
        rig.step(&mut part);
        let start = part.scene.as_ref().unwrap().stream;
        for _ in 0..8 {
            rig.step(&mut part);
        }
        assert_eq!(
            part.scene.as_ref().unwrap().stream - start,
            4,
            "eight ticks, four two-retrace frames"
        );
    }

    #[test]
    fn the_scene_is_the_u2e_city_with_the_name_hacks_object() {
        let (_, part) = started_with(closed());
        let scene = part.scene.as_ref().unwrap();
        assert_eq!(scene.master.city, 0, "U2E.00M byte 15 is 0");
        assert_eq!(scene.instances.len(), 58, "conum 58, camera first");
        let names: Vec<&[u8]> = scene
            .instances
            .iter()
            .map(|instance| instance.object.name.as_slice())
            .collect();
        assert!(names.contains(&b"\"s01\"".as_slice()), "U2E.028");
        // The objects whose quoted name puts `_` at name[1], which
        // `U2E.C:408` forces to 1e9: U2E.001, .003, .011, .014, .015, .016,
        // .017, .020 and .022 — nine of the 42 files.
        let underscored = scene
            .instances
            .iter()
            .filter(|instance| instance.object.name.get(1) == Some(&b'_'))
            .count();
        assert_eq!(underscored, 9, "the `_` objects U2E.C:408 forces to 1e9");
        // `currframe`'s window is strict: 1800 < currframe < 2200, in the
        // ring's frame units (`U2E.C:410-416`). The engine's own unit test
        // pins the sort; here the shipped scene shows the object the window
        // moves exists.
        assert!(scene.instances.iter().any(|instance| {
            instance.object.name.get(1..4) == Some(b"s01")
        }));
    }

    // --- the teardown -------------------------------------------------------

    #[test]
    fn the_teardown_fades_sixteen_steps_from_dac_entry_255_and_finishes() {
        let (mut rig, mut part) = started_with(opening_at(OPEN));
        to_loop(&mut rig, &mut part);
        // The live DAC is the main palette; the capture is that palette.
        let captured = part.main_palette.to_six_bit();
        // Tick T: the capture and the first +4 are computed, but the upload is
        // the next tick's (`fpal[a]+=4; dis_waitb(); outp(...)`,
        // `U2E.C:545-554`).
        let status = rig.step_exit(&mut part);
        assert_eq!(status, PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Teardown { left: TEARDOWN_STEPS - 1 });
        let mut step1 = captured;
        for byte in &mut step1 {
            *byte = (*byte + 4).min(63);
        }
        assert_eq!(part.fpal, step1, "the live DAC captured, then stepped");
        assert_eq!(rig.palette, part.main_palette, "no upload on the trigger tick");

        // Tick T+1: step 1 is the live DAC, written from entry 255 with the
        // wrap (255 gets fpal[0..3], entry j gets fpal[3(j+1)..3(j+2)]).
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        for index in 0..=255u8 {
            let at = if index == 255 {
                0
            } else {
                3 * (usize::from(index) + 1)
            };
            assert_eq!(
                rig.palette.entry(index),
                [step1[at], step1[at + 1], step1[at + 2]],
                "DAC entry {index}"
            );
        }
        let mut step2 = step1;
        for byte in &mut step2 {
            *byte = (*byte + 4).min(63);
        }
        assert_eq!(part.fpal, step2, "step 2 is computed one tick ahead");

        // Ticks T+2..T+16: each shows the previous step; the sixteenth upload
        // is the one the part ends on. No page is cleared.
        let before = rig.surface.pixels().to_vec();
        for step in 2..=TEARDOWN_STEPS {
            let status = rig.step(&mut part);
            if step < TEARDOWN_STEPS {
                assert_eq!(status, PartStatus::Continuing, "step {step}");
            } else {
                assert_eq!(status, PartStatus::Finished, "the sixteenth upload ends the part");
            }
        }
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(rig.surface.pixels(), before, "no page is cleared");
        let mut last = captured;
        for _ in 0..TEARDOWN_STEPS {
            for byte in &mut last {
                *byte = (*byte + 4).min(63);
            }
        }
        assert_eq!(part.fpal, last, "sixteen mutations");
        for index in 0..=255u8 {
            let at = if index == 255 {
                0
            } else {
                3 * (usize::from(index) + 1)
            };
            assert_eq!(
                rig.palette.entry(index),
                [last[at], last[at + 1], last[at + 2]],
                "the last upload is visible: DAC entry {index}"
            );
        }
        assert_eq!(rig.step(&mut part), PartStatus::Finished, "a finished part stays finished");
        assert_eq!(
            rig.surface.pixels(),
            before,
            "the no-op copper and the finished part touch nothing"
        );
    }

    #[test]
    fn a_sticky_exit_flag_runs_the_whole_sixteen_step_teardown() {
        // **The shell's exit flag is sticky** (`Engine::exit_requested`), so a
        // part that opted into draining sees it in every `PartCtx` of its
        // teardown. The source's fade is not guarded by `dis_exit` at all
        // (`U2E.C:541-555` runs after either loop exit), and every one of the
        // sixteen uploads must still happen: T's tick triggers the teardown and
        // computes step 1, T+1..T+16 upload steps 1..16, and the part returns
        // `Finished` on T+16.
        let (mut rig, mut part) = started_with(opening_at(OPEN));
        to_loop(&mut rig, &mut part);
        let captured = part.main_palette.to_six_bit();

        // Every tick from here carries the exit flag, exactly as the engine
        // hands it to a draining part's bands.
        assert_eq!(rig.step_exit(&mut part), PartStatus::Continuing, "T: teardown triggered");
        assert_eq!(part.phase, Phase::Teardown { left: TEARDOWN_STEPS - 1 });
        assert_eq!(rig.palette, part.main_palette, "T uploads nothing");

        let mut step = captured;
        for number in 1..=TEARDOWN_STEPS {
            // T+number: the previous tick's mutation is uploaded, then the next
            // is computed; the sixteenth is the one the part ends on.
            for byte in &mut step {
                *byte = (*byte + 4).min(63);
            }
            let status = rig.step_exit(&mut part);
            assert_eq!(
                rig.palette,
                rotated_palette(&step),
                "upload {number} of {TEARDOWN_STEPS} did not run"
            );
            if number < TEARDOWN_STEPS {
                assert_eq!(status, PartStatus::Continuing, "upload {number}");
            } else {
                assert_eq!(status, PartStatus::Finished, "the sixteenth upload ends the part");
            }
        }
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(part.fpal, step, "sixteen mutations, with the flag set throughout");
    }

    /// A diagnostic, not a commitment: render every stream state through the
    /// part's own driver and window and report which state each filed keyframe
    /// is. Run with `cargo test --release -p secondreality --bin secondreality
    /// search_reference_states -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn search_reference_states() {
        let mut scene = load_scene(SCENE_NAME).unwrap();
        scene.reset();
        let mut main = scene.master.palette;
        main.set_entry(252, [0, 0, 0]);
        main.set_entry(253, [63, 63, 63]);
        main.set_entry(254, [63, 63, 63]);
        main.set_entry(255, [0, 0, 0]);
        let references: Vec<(u32, Vec<u8>)> = [1000u32, 1600, 2600, 3600]
            .into_iter()
            .map(|index| (index, reference("u2e", index, "rgb")))
            .collect();
        let mut driver = Driver::new(Ring::new(Variant::U2e), false);
        let mut pixels = vec![0u8; PAGE_PIXELS];
        let mut best = vec![(usize::MAX, usize::MAX); references.len()];
        let mut counts = vec![Vec::new(); references.len()];
        for state in 0..=1800usize {
            if state > 0 {
                driver.fov = scene.step_frame().unwrap();
            }
            driver.ring.set_currframe((2 * state) as i32);
            {
                let mut page = Page::new(&mut pixels, usize::from(VIEW_WIDTH));
                driver.draw_frame(&mut scene, &mut page, Clear::Byte(255));
            }
            let rendered = rgb_of(&pixels, &main);
            for (at, (_, reference)) in references.iter().enumerate() {
                let differing = differing_pixels(&rendered, reference);
                counts[at].push(differing);
                if differing < best[at].0 {
                    best[at] = (differing, state);
                }
            }
        }
        for (at, (index, _)) in references.iter().enumerate() {
            let (count, state) = best[at];
            println!(
                "reference {index}: best state {state} with {count} differing; \
                 neighbours {}/{}/{}",
                counts[at][state.saturating_sub(1)],
                count,
                counts[at][(state + 1).min(1800)]
            );
        }

        // Attribute each differing pixel of the best state to the topmost
        // object that covers it, by drawing every object alone.
        for (at, (index, reference)) in references.iter().enumerate() {
            let state = best[at].1;
            let mut solo_scene = load_scene(SCENE_NAME).unwrap();
            solo_scene.reset();
            for _ in 0..state {
                solo_scene.step_frame().unwrap();
            }
            let mut solo_driver = Driver::new(Ring::new(Variant::U2e), false);
            solo_driver.fov = driver.fov;
            solo_driver.ring.set_currframe((2 * state) as i32);
            let on: Vec<bool> = solo_scene
                .instances
                .iter()
                .map(|instance| instance.on)
                .collect();
            let mut full = vec![0u8; PAGE_PIXELS];
            {
                let mut page = Page::new(&mut full, usize::from(VIEW_WIDTH));
                solo_driver.draw_frame(&mut solo_scene, &mut page, Clear::Byte(255));
            }
            let order = {
                for (other, instance) in solo_scene.instances.iter_mut().enumerate() {
                    instance.on = other != 0 && on[other];
                }
                solo_driver.order_for_test(&mut solo_scene)
            };
            let mut covers = std::collections::BTreeMap::new();
            for &index in &order {
                for (other, instance) in solo_scene.instances.iter_mut().enumerate() {
                    instance.on = other == index && on[index];
                }
                let mut pixels = vec![0u8; PAGE_PIXELS];
                {
                    let mut page = Page::new(&mut pixels, usize::from(VIEW_WIDTH));
                    solo_driver.draw_frame(&mut solo_scene, &mut page, Clear::Byte(255));
                }
                covers.insert(index, pixels);
            }
            let mut differing = Vec::new();
            for (at, &pixel) in full.iter().enumerate() {
                let theirs = &reference[3 * at..3 * at + 3];
                let full_entry = main.entry(pixel);
                if [dac_expand(full_entry[0]), dac_expand(full_entry[1]), dac_expand(full_entry[2])]
                    != [theirs[0], theirs[1], theirs[2]]
                {
                    differing.push(at);
                }
            }
            let mut by_object = std::collections::BTreeMap::new();
            let mut uncovered = 0usize;
            for &at in &differing {
                let top = order
                    .iter()
                    .rev()
                    .find(|index| covers[index][at] != 255)
                    .copied();
                match top {
                    Some(index) => *by_object.entry(index).or_insert(0usize) += 1,
                    None => uncovered += 1,
                }
            }
            let named: Vec<(usize, String, usize)> = by_object
                .iter()
                .map(|(&index, &count)| {
                    let name = String::from_utf8_lossy(&solo_scene.instances[index].object.name);
                    (index, name.to_string(), count)
                })
                .collect();
            let mut deltas = std::collections::BTreeMap::new();
            for &at in &differing {
                let theirs = &reference[3 * at..3 * at + 3];
                let matches: Vec<u8> = (0..=255u8)
                    .filter(|&candidate| {
                        let entry = main.entry(candidate);
                        [dac_expand(entry[0]), dac_expand(entry[1]), dac_expand(entry[2])]
                            == [theirs[0], theirs[1], theirs[2]]
                    })
                    .collect();
                if matches.len() != 1 {
                    *deltas.entry(i32::MIN).or_insert(0usize) += 1;
                    continue;
                }
                let delta = i32::from(matches[0]) - i32::from(full[at]);
                *deltas.entry(delta).or_insert(0usize) += 1;
            }
            println!(
                "reference {index} at state {state}: {} differing, topmost object {named:?}, \
                 uncovered {uncovered}, index deltas {deltas:?}",
                differing.len()
            );
        }
    }

    #[test]
    fn init_without_music_is_a_part_error() {
        let mut rig = rig(closed());
        let mut part = U2e::new();
        let error = {
            let mut ctx = rig.machine.ctx(None, 0, 0, 1, false, true);
            part.init(&mut ctx).unwrap_err()
        };
        assert_eq!(error.part, NAME);
        assert!(error.message.contains("no music"), "{}", error.message);
    }
}
