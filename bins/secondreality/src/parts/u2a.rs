//! U2A — the landing pad and the saucer, block 1's second part.
//!
//! `VISU/C/U2A.C` is the first of the vector engine's two parts. Its acts:
//!
//! 1. `:148-227` — load the packed scene: the `.00M` master with its palette,
//!    one object file per distinct number, the `.0AA` scene list and the first
//!    `.0AB` stream.
//! 2. `:229-240` — the first music gate: wait while
//!    `order <= 10 || row <= 46`.
//! 3. `:242-258` — the setup after the gate: the DAC upload, the window, the
//!    copper's slot 2 and the four-page ring.
//! 4. `:259-431` — the loop: wait for a free page, clear it with `U2ABG.UH`,
//!    order and draw the enabled objects, queue the page, then parse as many
//!    stream frames as the schedule says.
//! 5. `:432-451` — the teardown: slot 2 removed, all four pages cleared with
//!    the background.
//!
//! # The release's pre-gate pad
//!
//! The vendored tree's `main` draws nothing before the gate, but the shipped
//! release does: `reference/parts/u2a/frame_006500.rgb` is the `U2ABG.UH`
//! picture under a bright palette, `(3 * background + 5) & 63` with entry 195
//! black, from about boot 5966 to the gate (`docs/fidelity/u2a.md`; the
//! measurement is Task 1's, `.superpowers/sdd/06-vector-engine/task-1-report.md`
//! §4). The port paints that hold in `init` and leaves it until the gate, as
//! the controller's rulings record.
//!
//! # The tick map
//!
//! The gate is open at boot **6988** (`second1c`, order 11 row 47) and the
//! second at **7618** (`second1b`, order 12 row 55), but the stream ends first:
//! 521 drawable frames plus its END, at about boot 7509. The first gate's tick
//! is also the loop's first tick — `U2A.C:238-249` has no retrace between the
//! gate and the first draw — so nothing here models a separate setup tick.
//! `docs/fidelity/u2a.md` carries the measured map.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use sr_assets::load_path;
use sr_engine::copper::COPPER_SLOTS;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use super::visu::draw::Page;
use super::visu::{load_scene, Clear, Driver, Ring, Scene};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "u2a";

/// The packed scene directory and name (`U2A.C:148`, `:206`).
const SCENE_DIR: &str = "assets/VISU/C/SCENE";
const SCENE_NAME: &str = "U2A";
/// The background the parts clear with (`U2A.C:160`, `:285`).
const BACKGROUND_PATH: &str = "assets/VISU/C/U2ABG.UH";

/// The displayed picture: `vid_init(3)`'s oversampled 320x200 (`U2A.C:242`).
const VIEW_WIDTH: u16 = 320;
const VIEW_HEIGHT: u16 = 200;
/// One ring page's pixels (`U2A.C:90-95`, `:153-166`).
const PAGE_PIXELS: usize = 64_000;
/// The four-page ring (`cl[4]`, `U2A.C:90-94`).
const PAGES: usize = 4;
/// The surface pixels the four pages reach.
const SURFACE_PIXELS: usize = PAGES * PAGE_PIXELS;

/// The first gate (`U2A.C:238`): break when `order > 10 && row > 46`.
const GATE_ONE: (u8, u8) = (10, 46);
/// The second gate (`U2A.C:276`): break when `order > 11 && row > 54`.
const GATE_TWO: (u8, u8) = (11, 54);
/// The pad palette's brightening: `(3 * background + 5) & 63`.
const PAD_SCALE: u32 = 3;
const PAD_BIAS: u32 = 5;
/// The one measured pad entry off the formula: the white stars at index 195.
const PAD_BLACK: u8 = 195;

/// The merged palette's first background entry (`U2A.C:152`):
/// `memcpy(scene0+16+192*3, bg+16, 64*3)`.
const MERGE_FIRST: u8 = 192;

/// The measured release pad: `(3 * background + 5) & 63` (Task 1's report
/// §4; `docs/fidelity/u2a.md`). Every channel of every entry, entry 195
/// excepted.
fn pad_entry(background: [u8; 3]) -> [u8; 3] {
    [
        ((PAD_SCALE * u32::from(background[0]) + PAD_BIAS) & 63) as u8,
        ((PAD_SCALE * u32::from(background[1]) + PAD_BIAS) & 63) as u8,
        ((PAD_SCALE * u32::from(background[2]) + PAD_BIAS) & 63) as u8,
    ]
}

/// Where the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// Before the first gate: the release's bright pad is up, the ring does
    /// not exist and no copper is installed.
    Pad,
    /// The draw loop (`U2A.C:259-431`).
    Loop,
    /// The teardown has run and the part has ended.
    Done,
}

/// U2A: the scene, its background, the two palettes and the shared driver.
pub struct U2a {
    /// The loaded `.00M`/`.00N`/`.0AB` scene.
    scene: Option<Scene>,
    /// The `U2ABG.UH` pixels, linear 320x200: the clear image and the pad.
    background: Vec<u8>,
    /// The background's own 6-bit palette, the pad's and the merge's source.
    background_palette: Palette,
    /// `U2A.00M` entries 0-191 with the background's 0-63 at 192-255.
    merged_palette: Palette,
    /// `(3 * background + 5) & 63` over all 256 entries, 195 black.
    pad_palette: Palette,
    /// The ring and projection, shared with copper slot 2.
    driver: Option<Rc<RefCell<Driver>>>,
    phase: Phase,
    scene_dir: &'static str,
    background_path: &'static str,
}

impl U2a {
    /// A part with nothing loaded: `init` reads the scene and the background.
    pub fn new() -> U2a {
        U2a {
            scene: None,
            background: Vec::new(),
            background_palette: Palette::new(),
            merged_palette: Palette::new(),
            pad_palette: Palette::new(),
            driver: None,
            phase: Phase::Pad,
            scene_dir: SCENE_DIR,
            background_path: BACKGROUND_PATH,
        }
    }

    /// [`new`](U2a::new) reading its assets from elsewhere.
    #[cfg(test)]
    fn at(scene_dir: &'static str, background: &'static str) -> U2a {
        U2a {
            scene_dir,
            background_path: background,
            ..U2a::new()
        }
    }

    fn error(&self, message: String) -> PartError {
        PartError {
            part: NAME,
            message,
        }
    }

    /// Read the scene and the background, set the view and paint the
    /// release's pre-gate pad.
    ///
    /// The vendored `U2A.C` draws nothing before its gate, but the shipped
    /// release holds the `U2ABG.UH` picture under the measured pad palette
    /// from about boot 5966 to 6988 (`docs/fidelity/u2a.md`; Task 1's report
    /// §4). The view, the page and the DAC are set here so the hold is up
    /// from the part's first tick. The ring and its copper are *not* set up
    /// here: `U2A.C:249` installs `copper2` after the gate, so no retrace
    /// advances the ring during the hold.
    fn prepare(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        if ctx.music().is_none() {
            return Err(self.error(
                "this part waits on the music's order and row (U2A.C:229-238) and the run \
                 has no music to wait on"
                    .into(),
            ));
        }

        let scene = load_scene(Path::new(self.scene_dir), SCENE_NAME)
            .map_err(|message| self.error(message))?;
        let background = load_path(Path::new(self.background_path)).map_err(|error| {
            self.error(format!("{}: {error}", self.background_path))
        })?;
        if (background.width(), background.height()) != (VIEW_WIDTH, VIEW_HEIGHT) {
            return Err(self.error(format!(
                "{} is {}x{}; the part's clear image is {VIEW_WIDTH}x{VIEW_HEIGHT}",
                self.background_path,
                background.width(),
                background.height()
            )));
        }
        let background_palette = background.palette;
        let background_pixels = background.pixels().to_vec();
        if background_pixels.len() != PAGE_PIXELS {
            return Err(self.error(format!(
                "{} holds {} pixels; one page is {PAGE_PIXELS}",
                self.background_path,
                background_pixels.len()
            )));
        }
        if ctx.surface.pixels().len() < SURFACE_PIXELS {
            return Err(self.error(format!(
                "the four-page ring reaches pixel {SURFACE_PIXELS} and the surface holds {}",
                ctx.surface.pixels().len()
            )));
        }

        // `memcpy(scene0+16+192*3,bg+16,64*3)` (`U2A.C:152`): the scene's
        // entries 0..191 stay, the background's 0..63 land at 192..255.
        let mut merged_palette = scene.master.palette;
        for index in 0..64u8 {
            merged_palette.set_entry(MERGE_FIRST + index, background_palette.entry(index));
        }
        // The measured release pad: `(3 * background + 5) & 63` over all 256
        // entries, with the white stars' entry 195 black.
        let mut pad_palette = Palette::new();
        for index in 0..=255u8 {
            pad_palette.set_entry(index, pad_entry(background_palette.entry(index)));
        }
        pad_palette.set_entry(PAD_BLACK, [0, 0, 0]);

        // `vid_init(3)` is state only (`U2A.C:242`): the part runs in the
        // shipped 320x200 mode; `vid_window(0,319,25,174,512,9999999)`
        // (`:247`) is `Driver::new`'s window.
        ctx.view.width = VIEW_WIDTH;
        ctx.view.height = VIEW_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.y_line_offset = Some(VIEW_WIDTH);
        ctx.view.set_origin(0);
        ctx.view.pan = 0;
        ctx.view.split = None;
        ctx.view.border = None;

        // The release's hold is page 0 under the pad palette (`U2A.C:243-246`
        // is the source's post-gate DAC upload; the release moved the picture
        // and a measured palette in front of the gate).
        ctx.surface.pixels_mut()[..PAGE_PIXELS].copy_from_slice(&background_pixels);
        ctx.set_palette(pad_palette);

        self.scene = Some(scene);
        self.background = background_pixels;
        self.background_palette = background_palette;
        self.merged_palette = merged_palette;
        self.pad_palette = pad_palette;
        self.driver = None;
        self.phase = Phase::Pad;
        Ok(())
    }

    /// Whether the published position has passed `gate` (`U2A.C:238` and
    /// `:276`: `order > 10 && row > 46`, `order > 11 && row > 54`).
    fn open(ctx: &mut PartCtx, gate: (u8, u8)) -> bool {
        ctx.music().is_some_and(|music| {
            let position = music.position();
            position.order > gate.0 && position.row > gate.1
        })
    }

    /// `U2A.C:249-258`: install the ring's copper, in the same tick as the
    /// gate's first draw. The ring starts in the setup state (`Ring::new`),
    /// and from the end of this tick on every retrace runs `copper2`.
    fn install_ring(&mut self, ctx: &mut PartCtx) {
        let driver = Rc::new(RefCell::new(Driver::new(Ring::new(1), false)));
        let slot2 = Rc::clone(&driver);
        ctx.set_copper(
            2,
            Box::new(move |ctx: &mut PartCtx| {
                let mut driver = slot2.borrow_mut();
                // `copper2` (`U2A.C:105-133`), then
                // `vid_setswitch(-1,clr)`: the display start is the new
                // page's 64,000-pixel origin.
                driver.ring.isr();
                let origin = driver.ring.displayed() as u32 * PAGE_PIXELS as u32;
                ctx.view.set_origin(origin);
            }),
        );
        self.driver = Some(driver);
    }

    /// One iteration of `U2A.C:259-431`, except the loop's own conditions.
    fn loop_tick(&mut self, ctx: &mut PartCtx) {
        // `if(a>11 && b>54) break;` comes first (`U2A.C:276`).
        if Self::open(ctx, GATE_TWO) {
            self.teardown(ctx);
            return;
        }
        // `while(!dis_exit() && !xit)` (`U2A.C:259`).
        if ctx.exit_requested {
            self.teardown(ctx);
            return;
        }

        let Some(driver) = self.driver.as_ref().map(Rc::clone) else {
            return;
        };
        // `while(cl[clw].ready) { if(deadlock>16) break; }` (`U2A.C:278-283`):
        // a busy page makes this tick the wait, not a spin.
        let page = {
            let mut driver = driver.borrow_mut();
            driver.ring.next_page()
        };
        let Some(page) = page else {
            return;
        };

        // `vid_setswitch(clw,-1); vid_clearbg(bg); vid_cameraangle(fov);
        // order and draw; schedule and queue` (`U2A.C:284-336`).
        let parse = {
            let mut driver = driver.borrow_mut();
            let scene = self
                .scene
                .as_mut()
                .expect("the scene is loaded before the loop");
            let start = page * PAGE_PIXELS;
            {
                let mut page = Page::new(
                    &mut ctx.surface.pixels_mut()[start..start + PAGE_PIXELS],
                    usize::from(VIEW_WIDTH),
                );
                driver.draw_frame(scene, &mut page, Clear::Background(&self.background));
            }
            // `avgrepeat=(avgrepeat+(syncframe-currframe)+1)/2; ... queue;
            // currframe+=repeat;` (`U2A.C:327-336`).
            let (display, parse) = driver.ring.schedule();
            driver.ring.queue(page, display);
            parse
        };

        // `while(repeat-- && !xit) { parse one animation frame }`
        // (`U2A.C:336-430`): each frame's FOV becomes the next draw's angle,
        // and `0xFF 0xFF` resets the scene and sets `xit`.
        let mut xit = false;
        for _ in 0..parse {
            let angle = self
                .scene
                .as_mut()
                .expect("the scene is loaded before the loop")
                .step_frame();
            match angle {
                Some(angle) => driver.borrow_mut().fov = angle,
                None => {
                    xit = true;
                    break;
                }
            }
        }
        if xit || self.scene.as_ref().is_some_and(|scene| scene.xit) {
            self.teardown(ctx);
        }
    }

    /// `U2A.C:432-441`: slot 2 removed and all four pages cleared with the
    /// background. The source's `vid_setswitch(3,-1)` names page 3 as the
    /// **draw** page and leaves CRTC 0x0C (the display start) unchanged, so it
    /// never re-points the display; the port's page-3 origin is its own
    /// teardown ruling, and every page holds the same cleared background, so
    /// the choice is unobservable.
    fn teardown(&mut self, ctx: &mut PartCtx) {
        // The source removes slot 2 (`dis_setcopper(2,NULL)`); the port
        // installs no-ops in all three, the way `PAM` retires its copper, so
        // nothing an unregistered slot held can run again.
        for slot in 0..COPPER_SLOTS {
            ctx.set_copper(slot, Box::new(|_ctx: &mut PartCtx| {}));
        }
        let pixels = ctx.surface.pixels_mut();
        for page in 0..PAGES {
            let start = page * PAGE_PIXELS;
            pixels[start..start + PAGE_PIXELS].copy_from_slice(&self.background);
        }
        ctx.view.set_origin(((PAGES - 1) * PAGE_PIXELS) as u32);
        self.phase = Phase::Done;
    }
}

impl Part for U2a {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        self.prepare(ctx)
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Pad => {
                // `_asm int 0fch` (`U2A.C:231-237`) once a tick. The gate is
                // tested before `dis_exit` (`:238-239`), so a tick where the
                // gate opens *and* the exit is set still runs the setup and
                // then the loop's own exit check, which tears down.
                if !Self::open(ctx, GATE_ONE) {
                    // `if(dis_exit()) return;` (`:239`): a closed gate with
                    // the exit set leaves the pad and the DIS retires the
                    // part; there is no copper to remove and no page to
                    // clear.
                    if ctx.exit_requested {
                        self.phase = Phase::Done;
                        return PartStatus::Finished;
                    }
                    return PartStatus::Continuing;
                }
                // The gate's tick is the loop's first tick: the DAC upload
                // (`U2A.C:243-246`), the copper, the ring and the first draw
                // all happen here, with no retrace between them.
                ctx.set_palette(self.merged_palette);
                self.install_ring(ctx);
                self.phase = Phase::Loop;
                self.loop_tick(ctx);
                if self.phase == Phase::Done {
                    PartStatus::Finished
                } else {
                    PartStatus::Continuing
                }
            }
            Phase::Loop => {
                self.loop_tick(ctx);
                if self.phase == Phase::Done {
                    PartStatus::Finished
                } else {
                    PartStatus::Continuing
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::visu::math::Matrix;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{Music, MusicPosition};
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::view::View;
    use sr_engine::{dac_expand, Surface};

    const TEST_SCENE_DIR: &str = "../../assets/VISU/C/SCENE";
    const TEST_BACKGROUND: &str = "../../assets/VISU/C/U2ABG.UH";
    const TEST_REFERENCE_6500: &str = "../../reference/parts/u2a/frame_006500.rgb";
    const TEST_REFERENCE_7300: &str = "../../reference/parts/u2a/frame_007300.rgb";
    const TEST_REFERENCE_7640: &str = "../../reference/parts/u2a/frame_007640.rgb";
    /// The 7640 index plane: the `U2ABG.UH` image under the natural palette
    /// (`docs/fidelity/u2a.md` §4). The 6500 frame is the same pixels under
    /// the pad palette, so the pair measures the palette entry by entry.
    const TEST_REFERENCE_7640_IDX: &str = "../../reference/parts/u2a/frame_007640.idx";

    /// The music a unit test hands the part: one scripted published position
    /// a tick, every other call the null reading.
    struct Player {
        positions: Vec<MusicPosition>,
        at: usize,
    }

    impl Player {
        fn scripted(positions: Vec<(u8, u8)>) -> Player {
            Player {
                positions: positions
                    .into_iter()
                    .map(|(order, row)| MusicPosition {
                        order,
                        row,
                        ..Default::default()
                    })
                    .collect(),
                at: 0,
            }
        }

        fn constant(order: u8, row: u8) -> Player {
            Player::scripted(vec![(order, row)])
        }

        /// The gate's open position (`second1c`'s 6988: order 11 row 47).
        fn open() -> Player {
            Player::constant(GATE_ONE.0 + 1, GATE_ONE.1 + 1)
        }

        /// The gate's closed position (`second1c`'s 6987: order 11 row 46).
        fn closed() -> Player {
            Player::constant(GATE_ONE.0 + 1, GATE_ONE.1)
        }
    }

    impl Music for Player {
        fn poll(&mut self) {}
        fn position(&self) -> MusicPosition {
            self.positions
                .get(self.at)
                .copied()
                .unwrap_or_else(|| *self.positions.last().unwrap_or(&MusicPosition::default()))
        }
        fn musplus(&mut self) -> i32 {
            -32
        }
        fn musrow(&mut self) -> i32 {
            self.position().row as i32
        }
        fn muscode(&mut self, _wanted: i32) -> i32 {
            0
        }
        fn sync(&mut self) -> i32 {
            0
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

        fn ctx_with(&mut self, tick: u32, exit: bool) -> PartCtx<'_> {
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
                false,
            )
        }

        fn ctx(&mut self, tick: u32) -> PartCtx<'_> {
            self.ctx_with(tick, false)
        }

        /// A context for a run with no player at all.
        fn ctx_without_music(&mut self, tick: u32) -> PartCtx<'_> {
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                None,
                tick,
                0,
                1,
                true,
                false,
                false,
            )
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

        /// One tick as the engine orders it: slot 0, the part, the compose,
        /// slots 1 and 2.
        fn step_render(&mut self, part: &mut U2a) -> (PartStatus, Vec<u8>) {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx(tick));
            let frame = self.rendered_rgb();
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            (status, frame)
        }

        fn step(&mut self, part: &mut U2a) -> PartStatus {
            self.step_render(part).0
        }

        fn step_exit(&mut self, part: &mut U2a) -> PartStatus {
            self.run_slot(0);
            let tick = self.tick;
            let status = part.frame(&mut self.ctx_with(tick, true));
            self.run_slot(1);
            self.run_slot(2);
            self.tick += 1;
            status
        }

        /// The frame the engine would compose: the view's window through the
        /// live palette, widened as the DAC widens.
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

        fn page(&self, page: usize) -> &[u8] {
            let start = page * PAGE_PIXELS;
            &self.surface.pixels()[start..start + PAGE_PIXELS]
        }
    }

    fn started_with(player: Player) -> (Rig, U2a) {
        let mut rig = Rig::new(player);
        let mut part = U2a::at(TEST_SCENE_DIR, TEST_BACKGROUND);
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    fn reference(path: &str) -> Vec<u8> {
        std::fs::read(path).unwrap()
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

    // --- the brief's tests --------------------------------------------------

    #[test]
    fn init_sets_the_shipped_view_and_paints_the_pad() {
        let (rig, part) = started_with(Player::closed());
        assert_eq!(part.phase, Phase::Pad);
        assert_eq!((rig.view.width, rig.view.height), (VIEW_WIDTH, VIEW_HEIGHT));
        assert_eq!(rig.view.start_addr, 0, "page 0 is displayed");
        assert_eq!(rig.view.y_line_offset, Some(320), "CRTC 0x13: 320 pixels a line");
        assert!(!rig.view.double_scan);
        assert_eq!(rig.page(0), part.background, "page 0 holds U2ABG.UH");
        assert_eq!(rig.palette, part.pad_palette, "the measured pad palette is up");
        assert!(!rig.copper.is_set(0));
        assert!(!rig.copper.is_set(1));
        assert!(!rig.copper.is_set(2), "the ring's copper is installed at the gate");
        assert!(part.driver.is_none());
    }

    #[test]
    fn the_pad_palette_is_the_measured_formula_with_entry_195_black() {
        let (rig, part) = started_with(Player::closed());
        for index in 0..=255u8 {
            let expected = if index == PAD_BLACK {
                [0, 0, 0]
            } else {
                pad_entry(part.background_palette.entry(index))
            };
            assert_eq!(rig.palette.entry(index), expected, "pad entry {index}");
        }
        // The one measured entry off the formula: U2ABG entry 3 is 63 and
        // would brighten to 2, but the reference's white stars are black.
        assert_eq!(part.background_palette.entry(PAD_BLACK), [63, 63, 63]);
        assert_eq!(pad_entry([63, 63, 63]), [2, 2, 2]);

        // A32's counts, measured rather than quoted: the reference's own
        // 7640 index plane says which pixels use which entry, and the 6500
        // frame — the same picture under the pad palette — says what the
        // release's DAC showed there. Of the 57,600 pixels whose index is
        // >= 192, the formula reproduces 57,516; the other 84 are the
        // index-195 stars, which the release renders black.
        let indices = reference(TEST_REFERENCE_7640_IDX);
        let measured = reference(TEST_REFERENCE_6500);
        let pixels = usize::from(VIEW_WIDTH) * usize::from(VIEW_HEIGHT);
        assert_eq!(indices.len(), pixels, "the 7640 index plane is one byte a pixel");
        assert_eq!(measured.len(), pixels * 3, "the 6500 frame is one RGB triple a pixel");
        let mut above_192 = 0usize;
        let mut stars = 0usize;
        let mut formula_matches = 0usize;
        for (&index, rgb) in indices.iter().zip(measured.chunks_exact(3)) {
            if index < 192 {
                continue;
            }
            above_192 += 1;
            if index == PAD_BLACK {
                stars += 1;
            }
            // The background's own entries 0..63 are the palette the release
            // transformed; 192..255 index them (`(3 * bg + 5) & 63`).
            let entry = pad_entry(part.background_palette.entry(index - 192));
            let expected = [
                dac_expand(entry[0]),
                dac_expand(entry[1]),
                dac_expand(entry[2]),
            ];
            if rgb == expected {
                formula_matches += 1;
            }
        }
        assert_eq!(above_192, 57_600, "the pixels that use indices >= 192");
        assert_eq!(stars, 84, "the index-195 star pixels");
        assert_eq!(formula_matches, 57_516, "the pixels the formula reproduces");
    }

    #[test]
    fn the_merged_palette_is_the_scene_then_the_background() {
        let (_, part) = started_with(Player::closed());
        let scene = part.scene.as_ref().unwrap();
        let mut expected = scene.master.palette;
        for index in 0..64u8 {
            expected.set_entry(MERGE_FIRST + index, part.background_palette.entry(index));
        }
        assert_eq!(part.merged_palette, expected);
        assert_eq!(
            part.merged_palette.entry(MERGE_FIRST),
            part.background_palette.entry(0),
            "entry 192 is the background's entry 0 (U2A.C:152)"
        );
        assert_eq!(
            part.merged_palette.entry(MERGE_FIRST - 1),
            scene.master.palette.entry(MERGE_FIRST - 1),
            "entry 191 is still the scene's"
        );
        assert_ne!(
            scene.master.palette.entry(MERGE_FIRST),
            part.background_palette.entry(0),
            "the merge is observable: the scene's 192 is not black"
        );
    }

    #[test]
    fn the_gate_holds_until_the_scripted_cue_and_exits_on_request() {
        let (mut rig, mut part) = started_with(Player::scripted(vec![
            (GATE_ONE.0, GATE_ONE.1),
            (GATE_ONE.0 + 1, GATE_ONE.1),
            (GATE_ONE.0 + 1, GATE_ONE.1 + 1),
        ]));
        let pad_page = rig.page(0).to_vec();
        let pad_palette = rig.palette;
        // Row 46 is not `> 46`, and order 10 is not `> 10`: two closed ticks.
        for _ in 0..2 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
            assert_eq!(part.phase, Phase::Pad);
            assert_eq!(rig.page(0), pad_page, "the pad holds");
            assert_eq!(rig.palette, pad_palette, "the pad DAC holds");
            assert!(part.driver.is_none(), "no ring before the gate");
            assert!(!rig.copper.is_set(2), "no copper before the gate");
        }
        // Tick 2 is open: the merged palette, the ring and the first draw, in
        // one tick (`U2A.C:238-249`).
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::Loop);
        assert_eq!(rig.palette, part.merged_palette, "the merged scene palette is up");
        assert!(part.driver.is_some());
        assert!(rig.copper.is_set(2), "the ring's copper is installed");
        let scene = part.scene.as_ref().unwrap();
        assert_eq!(scene.stream, 1, "the first loop tick parsed stream frame 0");
        assert!(scene.instances.iter().all(|instance| !instance.on));

        // `dis_exit()` during the gate returns without touching the screen
        // (`U2A.C:239`).
        let (mut rig, mut part) = started_with(Player::closed());
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        // `U2A.C:239`'s bare `return` leaves the pad: no copper, no clear.
        assert_eq!(rig.page(0), part.background, "the pad screen stays up");
        assert_eq!(rig.palette, part.pad_palette, "and the pad DAC");
        assert!(!rig.copper.is_set(2), "no copper was installed");
        assert_eq!(rig.step(&mut part), PartStatus::Finished, "a finished part stays finished");

        // The gate is tested first (`U2A.C:238-239`): an open gate with the
        // exit set runs the setup and then the loop's own exit check, which
        // tears down to the cleared page.
        let (mut rig, mut part) = started_with(Player::open());
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        for page in 0..PAGES {
            assert_eq!(rig.page(page), part.background, "page {page} cleared");
        }
        assert_eq!(rig.view.start_addr, 3 * PAGE_PIXELS as u32);
    }

    #[test]
    fn the_second_gate_tears_down_on_the_first_tick_it_is_open() {
        // `if(a>11 && b>54) break;` (`U2A.C:276`) is the loop's first test. It
        // never fires in the shipped data — the stream's END at 7510 comes
        // before order 12 row 55 at 7618 (`second1b`) — so a scripted position
        // is the only way to drive it.
        let (mut rig, mut part) = started_with(Player::constant(GATE_TWO.0 + 1, GATE_TWO.1 + 1));
        assert_eq!(rig.step(&mut part), PartStatus::Finished);
        assert_eq!(part.phase, Phase::Done);
        for page in 0..PAGES {
            assert_eq!(rig.page(page), part.background, "page {page} cleared");
        }
        assert_eq!(rig.view.start_addr, 3 * PAGE_PIXELS as u32);
    }

    #[test]
    fn init_without_music_is_a_part_error() {
        // PAM's precedent: a part whose gates read the published position
        // refuses to start when the run has no player, rather than waiting
        // for ever.
        let mut rig = Rig::new(Player::closed());
        let mut part = U2a::at(TEST_SCENE_DIR, TEST_BACKGROUND);
        let error = {
            let mut ctx = rig.ctx_without_music(0);
            part.init(&mut ctx).unwrap_err()
        };
        assert_eq!(error.part, NAME);
        assert!(
            error.message.contains("no music"),
            "the refusal names the reason: {}",
            error.message
        );
    }

    #[test]
    fn the_first_stream_frame_leaves_every_object_off_and_frame_sixteen_turns_object_two_on() {
        let (mut rig, mut part) = started_with(Player::open());
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        {
            let scene = part.scene.as_ref().unwrap();
            assert_eq!(scene.stream, 1, "frame 0 is parsed");
            assert!(
                scene.instances.iter().all(|instance| !instance.on),
                "frame 0 has no toggle"
            );
            assert_ne!(scene.cam, Matrix::default(), "frame 0 moves the camera");
        }
        assert_eq!(part.driver.as_ref().unwrap().borrow().fov, 0x2200);
        // Frame 16 turns object 2 (U2A.002) on; nothing later toggles it off.
        let mut saw_off = false;
        for _ in 0..120 {
            let status = rig.step(&mut part);
            let scene = part.scene.as_ref().unwrap();
            if scene.stream <= 16 {
                assert!(!scene.instances[2].on, "stream {} still off", scene.stream);
                saw_off = true;
            } else if scene.stream > 16 {
                assert!(scene.instances[2].on, "frame 16 turned object 2 on");
            }
            assert_eq!(part.driver.as_ref().unwrap().borrow().fov, 0x2200);
            if status == PartStatus::Finished {
                break;
            }
        }
        assert!(saw_off, "the stream was observed before frame 16");
        assert!(part.scene.as_ref().unwrap().instances[2].on);
    }

    #[test]
    fn teardown_clears_all_four_pages_and_leaves_page_three_displayed() {
        let (mut rig, mut part) = started_with(Player::open());
        let mut finished_at = None;
        for step in 0..600u32 {
            if rig.step(&mut part) == PartStatus::Finished {
                finished_at = Some(step);
                break;
            }
        }
        // 521 drawable frames plus END; the parse is one frame a tick in the
        // steady state, and the END lands on the 522nd parse.
        assert_eq!(finished_at, Some(521), "the stream's END tick, counting the gate tick as 0");
        assert_eq!(part.phase, Phase::Done);
        let scene = part.scene.as_ref().unwrap();
        assert!(scene.xit);
        assert_eq!(scene.stream, 0, "resetscene rewound the stream");
        for page in 0..PAGES {
            assert_eq!(rig.page(page), part.background, "page {page} cleared");
        }
        assert_eq!(rig.view.start_addr, 3 * PAGE_PIXELS as u32, "the last cleared page");
        assert_eq!(rig.palette, part.merged_palette, "the DAC keeps the merged palette");
        for slot in 0..COPPER_SLOTS {
            assert!(rig.copper.is_set(slot), "slot {slot} holds a no-op");
        }
        // The no-ops do nothing: a finished part's screen holds.
        let before = rig.surface.pixels().to_vec();
        rig.run_slot(0);
        rig.run_slot(1);
        rig.run_slot(2);
        assert_eq!(rig.surface.pixels(), before, "the no-op copper changes nothing");
        assert_eq!(rig.view.start_addr, 3 * PAGE_PIXELS as u32);
        // The cleared hold under the merged palette is the reference's 7640.
        let held = rig.rendered_rgb();
        let differing = differing_pixels(&held, &reference(TEST_REFERENCE_7640));
        assert_eq!(differing, 0, "the cleared hold is not the reference's");
    }

    #[test]
    fn the_first_displayed_page_is_the_cleared_background() {
        let (mut rig, mut part) = started_with(Player::open());
        let (_, frame) = rig.step_render(&mut part);
        assert_eq!(
            part.driver.as_ref().unwrap().borrow().ring.displayed(),
            0,
            "the ring starts displaying page 0"
        );
        assert_eq!(frame, plain_rgb(&part.background, &part.merged_palette));
        assert_eq!(rig.page(0), part.background);

        // The pad before the gate is the same pixels under the pad palette.
        let (mut rig, mut part) = started_with(Player::closed());
        let (_, frame) = rig.step_render(&mut part);
        assert_eq!(frame, plain_rgb(&part.background, &part.pad_palette));
    }

    #[test]
    fn the_pad_hold_is_the_reference_6500() {
        let (mut rig, mut part) = started_with(Player::closed());
        let (_, frame) = rig.step_render(&mut part);
        let differing = differing_pixels(&frame, &reference(TEST_REFERENCE_6500));
        assert_eq!(differing, 0, "the pad frame is not the reference's");
    }

    /// One image through one palette, widened as the engine widens it.
    fn plain_rgb(pixels: &[u8], palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(pixels.len() * 3);
        for &index in pixels {
            let entry = palette.entry(index);
            out.push(dac_expand(entry[0]));
            out.push(dac_expand(entry[1]));
            out.push(dac_expand(entry[2]));
        }
        out
    }

    #[test]
    fn the_reference_7300_is_the_measured_stream_state() {
        // D25's calibration for the moving keyframe: render every stream
        // state through the same driver and shipped window the part uses, and
        // record which state the reference's picture is. The port's own tick
        // map decides when that state reaches the screen; this pins what it
        // looks like.
        let mut scene = load_scene(Path::new(TEST_SCENE_DIR), SCENE_NAME).unwrap();
        let background = load_path(Path::new(TEST_BACKGROUND)).unwrap();
        let mut merged = scene.master.palette;
        for index in 0..64u8 {
            merged.set_entry(MERGE_FIRST + index, background.palette.entry(index));
        }
        let mut driver = Driver::new(Ring::new(1), false);
        let mut pixels = vec![0u8; PAGE_PIXELS];
        let reference_7300 = reference(TEST_REFERENCE_7300);
        let mut best = (usize::MAX, usize::MAX);
        let mut best_pixels = Vec::new();
        let mut neighbours = std::collections::BTreeMap::new();
        let mut counts = vec![usize::MAX; 522];
        for (state, count) in counts.iter_mut().enumerate() {
            if state > 0 {
                driver.fov = scene.step_frame().unwrap();
            }
            {
                let mut page = Page::new(&mut pixels, usize::from(VIEW_WIDTH));
                driver.draw_frame(&mut scene, &mut page, Clear::Background(background.pixels()));
            }
            let differing = differing_pixels(&plain_rgb(&pixels, &merged), &reference_7300);
            *count = differing;
            if [293, 294, 295].contains(&state) {
                neighbours.insert(state, differing);
            }
            if differing < best.0 {
                best = (differing, state);
                best_pixels = pixels.clone();
            }
        }
        println!(
            "reference 7300: best stream state {} with {} differing pixels; 293/294/295 = \
             {neighbours:?}",
            best.1, best.0
        );
        assert_eq!(best.1, 294, "the reference's saucer pose is stream state 294");
        assert_eq!(best.0, 426, "the measured residual at that state");
        assert_eq!(
            neighbours.get(&293),
            Some(&892),
            "the state before the reference's"
        );
        assert_eq!(
            neighbours.get(&295),
            Some(&858),
            "the state after the reference's"
        );
        // The live run's boot-7300 frame differs from the same reference by
        // 1098 (`tests/u2a.rs`); against the reference that count belongs to
        // stream state 297 and to nothing else, which is what identifies the
        // port's pose as state 297 (A29).
        let at_1098: Vec<usize> = (0..=521).filter(|&state| counts[state] == 1098).collect();
        println!("states whose reference comparison is 1098: {at_1098:?}");
        assert_eq!(counts[297], 1098);
        assert_eq!(at_1098, vec![297], "1098 uniquely identifies stream state 297");

        // The silhouette is exact: every pixel the object covers and every
        // background pixel agrees with the reference. The residual is inside
        // the object, on its Gouraud-shaded faces (docs/fidelity/u2a.md §5).
        let reference_idx = std::fs::read("../../reference/parts/u2a/frame_007300.idx").unwrap();
        let mut mask_differing = 0;
        let mut deltas = std::collections::BTreeSet::new();
        for (at, (&ours, &theirs)) in best_pixels.iter().zip(&reference_idx).enumerate() {
            let background_pixel = background.pixels()[at];
            if (ours != background_pixel) != (theirs != background_pixel) {
                mask_differing += 1;
            }
            if ours != theirs {
                deltas.insert(i32::from(theirs) - i32::from(ours));
            }
        }
        println!("7300 silhouette: {mask_differing} differing pixels; index deltas {deltas:?}");
        assert_eq!(mask_differing, 0, "the object's silhouette is the reference's");
        assert_eq!(
            deltas.into_iter().collect::<Vec<_>>(),
            vec![-5, -4, -3, -2, -1, 1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12],
            "the residual is the measured small-index spread"
        );

        // Attribute every differing pixel to the topmost object that writes
        // it, by drawing each object alone at the same state. The paint order
        // is the z sort's: U2A.002 (flat, `F_SHADE16`) first, then the three
        // U2A.001 copies (each face Gouraud, `F_SHADE32`).
        let mut alone_scene = load_scene(Path::new(TEST_SCENE_DIR), SCENE_NAME).unwrap();
        for _ in 0..294 {
            alone_scene.step_frame();
        }
        let mut alone = std::collections::BTreeMap::new();
        for index in [1usize, 2, 4, 5] {
            for other in [1usize, 2, 4, 5] {
                alone_scene.instances[other].on = other == index;
            }
            let mut solo = vec![0u8; PAGE_PIXELS];
            {
                let mut page = Page::new(&mut solo, usize::from(VIEW_WIDTH));
                driver.draw_frame(
                    &mut alone_scene,
                    &mut page,
                    Clear::Background(background.pixels()),
                );
            }
            alone.insert(index, solo);
        }
        let mut by_instance = [0usize; 6];
        for (at, (&ours, &theirs)) in best_pixels.iter().zip(&reference_idx).enumerate() {
            if ours == theirs {
                continue;
            }
            if let Some(&top) = [5usize, 4, 1, 2]
                .iter()
                .find(|index| alone[index][at] != background.pixels()[at])
            {
                by_instance[top] += 1;
            }
        }
        println!("7300 residual by topmost object: {by_instance:?}");
        assert_eq!(
            by_instance,
            [0, 75, 0, 0, 115, 236],
            "the residual is on the Gouraud-shaded U2A.001 copies; the flat \
             U2A.002 reproduces exactly (docs/fidelity/u2a.md §5)"
        );
    }
}
