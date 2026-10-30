//! PLZPART — the plasma and the cube: one part, two screens.
//!
//! `PLZPART/MAIN.C` runs `init_copper(); initvect(); plz(); vect();
//! close_copper()` over `PLZ.C` (the plasma), `VECT.C` and `PLZFILL.C` (the
//! cube), `PLZA.ASM` (the span rasteriser and the clear), `COPPER.ASM` (three
//! copper routines), `ASMYT.ASM` (`plzline`), `SPLINE.ASM` (the camera spline)
//! and `TWEAK.ASM` (the two mode sets). The stale `MAIN.ASM` is not linked.
//!
//! - [`tables`]: the `.INC` fragments and the segment `plzline` reads.
//! - [`release`]: the camera path and sine table read from the shipped image.
//! - [`copper`]: the copper routines, the palette tables and the drop.
//! - [`plasma`]: `plz()`, 320×400 behind a line-compare split.
//! - [`cube`]: `vect()`, the release's geometry on a 134-row screen.
//! - [`raster`]: `do_poly`, the span rasteriser and the clear.
//!
//! Tick 0 is boot 5723, where the part's `setmframe(0)` (`PLZ.C:69`) lands (a
//! mapping, not an anchor); a fresh run passes the plasma's music gate there,
//! with the music wait's parameter walks made in `init`.
//!
//! **Preserved** (`bugs.md` §1, §17): the 84-byte `plzline` row in a 96-byte
//! stride (`ASMYT.ASM:41-69`); `do_clear`'s `end_mask[x-1]` underflow at
//! `x = 0` (`PLZA.ASM:350-363`); `memset(fadepal,0,768)` clearing only the
//! colour half (`PLZ.C:81`); the drop's last step cutting the last fade short;
//! `dtau` reaching 404 (`COPPER.ASM:175-181`), an inert split 405; the double
//! `+160` (`VECT.C:198`); the dead `timetable[4] = 2778`; `count_const`'s
//! subtraction inside the final shift. **Release values** (D24): the plasma
//! exits at `curpal == 4`, not 5 (A22); the cube's data and geometry are the
//! image's (A23); the plasma's tear is not reproduced (A24). Measurements:
//! `docs/fidelity/plzpart.md`.

use std::cell::RefCell;
use std::rc::Rc;

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

use crate::parts::read_asset;

mod copper;
mod cube;
mod plasma;
mod raster;
mod release;
mod tables;

use copper::{build_pals, CopperState};
use cube::{CUBE_FACES, CUBE_POINTS};
use plasma::{PLASMA_COP_START, PLASMA_ROW_BYTES, PLASMA_WAIT_WALKS, PLASMA_WIDTH};
use raster::{build_masks, Span, CLR_SLOTS};
use release::{release_cube, EXE_PATH};
use tables::Tables;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "plzpart";

/// The entry run's head start, in polls: after the shell's seek to order 62
/// (`music_for_part`), eight put the cube's `musplus() >= 13` gate on manifest
/// 2641 = boot 8364, the trace's first order-68 row 13; nine are one early
/// (`the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`).
const MUSIC_PRE_ROLL: u32 = 8;
/// `VECT.C:116`: `while(dis_musplus()<13 ...)`.
const CUBE_GATE: i32 = 13;

/// Where the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// The plasma's `while(dis_musplus()<0)` gate (`PLZ.C:68`). A fresh run
    /// passes it at tick 0: the shell's seek to order 62 stands where the
    /// reference's gate passed (`plzpart.md` §8).
    PlasmaGate,
    /// `PLZ.C:76-118`: the plasma's frame loop.
    Plasma,
    /// `PLZ.C:119`: `while(frame_count==0);` — one tick.
    PlasmaEndWait,
    /// `VECT.C:114-116`: the mode set, then the cube's music gate.
    CubeGate,
    /// `VECT.C:117-128`: `calculate(1); draw(15); clear();` a tick. The page
    /// for this iteration is already swapped (the gate's `swappage` is the
    /// loop's first), so there is no unswapped state to enter with.
    Cube,
    /// `VECT.C:119-120`: the cube's exit.
    Done,
}

/// PLZPART: the plasma and the cube.
pub struct Plzpart {
    tables: Option<Tables>,
    /// Everything the three copper routines own, shared with their closures.
    copper: Rc<RefCell<CopperState>>,
    /// The part's phase.
    phase: Phase,
    /// The part's own tick.
    tick: u32,
    /// Whether this run's player was installed for this part (`music_fresh`).
    fresh: bool,

    // ---- the cube ----
    /// The release's sine segment (`sinit`, 10,240 words).
    sinit: Vec<i16>,
    /// The release's camera control points (`buu`, 1,088 words).
    buu: Vec<i16>,
    /// The four-plane linear image.
    video: Vec<u8>,
    /// `PLZA.ASM:13-20`: the two periodic plane masks, flattened.
    masks: Vec<u8>,
    /// `PLZFILL.C:12`: the runtime sine table, 2,000 `int`s.
    sini: Vec<i16>,
    /// `PLZFILL.C:21`: the cube's own palette, 768 bytes.
    pal: [u8; 768],
    kuva1: Vec<u8>,
    kuva2: Vec<u8>,
    kuva3: Vec<u8>,
    dist1: Vec<u8>,
    /// `PLZFILL.C:22,24`: the eight edge-list slots and the rotation cursor.
    clrtau: Vec<[i32; 2]>,
    clrptr: usize,
    /// `VECT.C:74-92`: the object, its projected points and the draw list.
    points: [[i32; 3]; 8],
    xx: [[i32; 3]; 8],
    xxx: [[i32; 2]; 8],
    pg: [[i32; 8]; 6],
    ptodraw: [i32; 256],
    polys: i32,
    light_src: [i32; 6],
    lls: [i32; 6],
    cxx: i32,
    cxy: i32,
    cxz: i32,
    cyx: i32,
    cyy: i32,
    cyz: i32,
    czx: i32,
    czy: i32,
    czz: i32,
    kx: i32,
    ky: i32,
    kz: i32,
    dis: i32,
    tx: i32,
    ty: i32,
    ls_kx: i32,
    ls_ky: i32,
    ls_x: i32,
    ls_y: i32,
    ls_z: i32,
    page: usize,
    frames: i32,
    /// The rasteriser's live span state.
    span: Span,
}

impl Plzpart {
    /// A part with nothing loaded: `init` reads the five fragments and the shipped image.
    pub fn new() -> Plzpart {
        Plzpart {
            tables: None,
            copper: Rc::new(RefCell::new(CopperState::new())),
            phase: Phase::PlasmaGate,
            tick: 0,
            fresh: false,
            sinit: Vec::new(),
            buu: Vec::new(),
            video: Vec::new(),
            masks: build_masks(),
            sini: Vec::new(),
            pal: [0; 768],
            kuva1: Vec::new(),
            kuva2: Vec::new(),
            kuva3: Vec::new(),
            dist1: Vec::new(),
            clrtau: vec![[640, 0]; CLR_SLOTS * 256],
            clrptr: 0,
            points: CUBE_POINTS,
            xx: [[0; 3]; 8],
            xxx: [[0; 2]; 8],
            pg: CUBE_FACES,
            ptodraw: [0; 256],
            polys: 0,
            light_src: [0; 6],
            lls: [0; 6],
            cxx: 0,
            cxy: 0,
            cxz: 0,
            cyx: 0,
            cyy: 0,
            cyz: 0,
            czx: 0,
            czy: 0,
            czz: 0,
            kx: 0,
            ky: 0,
            kz: 0,
            dis: 320,
            tx: 0,
            ty: -50,
            ls_kx: 0,
            ls_ky: 0,
            ls_x: 0,
            ls_y: 0,
            ls_z: 0,
            page: 0,
            frames: 0,
            span: Span::default(),
        }
    }
}

impl Part for Plzpart {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // The part gates on the music (`PLZ.C:68`, `VECT.C:116,119`); a run
        // with no player could never leave the gate.
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (PLZPART/PLZ.C:68, VECT.C:116) and the run has no music to wait on",
        )?;
        let tables = Tables::load()?;
        let pals = build_pals(&tables);
        {
            let mut state = self.copper.borrow_mut();
            state.pals = pals;
        }
        let error = |message: String| PartError::new(NAME, message);
        let file = read_asset(NAME, EXE_PATH)?;
        let image = sr_assets::exe::unpack(&file).map_err(|e| error(format!("{EXE_PATH}: {e}")))?;
        let release = release_cube(&image).map_err(|e| error(format!("{EXE_PATH}: {e}")))?;
        self.sinit = release.sinit;
        self.buu = release.buu;
        self.tables = Some(tables);
        self.initvect();

        // The plasma's surface is the four-plane image at four pixels a byte;
        // the window reaches 4*(27,072 + 61*96) + 320.
        let needed =
            4 * (PLASMA_COP_START as usize + 61 * PLASMA_ROW_BYTES) + PLASMA_WIDTH as usize;
        if ctx.surface.pixels().len() < needed {
            return Err(PartError::new(
                NAME,
                format!(
                    "the plasma's window reaches surface pixel {needed} and the surface holds {}",
                    ctx.surface.pixels().len()
                ),
            ));
        }

        // A mid-chain start has already run through the head start's polls.
        self.fresh = ctx.music_fresh();
        if self.fresh {
            if let Some(music) = ctx.music() {
                for _ in 0..MUSIC_PRE_ROLL {
                    music.poll();
                }
            }
        }
        // `MAIN.C:8-9`: `init_copper()` runs before `plz()`, so the copper is
        // live through the music wait. Register it here; the fresh run's wait
        // is the measured parameter pre-walk below, a mid-chain run's is the
        // source's `musplus` gate (whose ticks then walk the parameters).
        self.install_copper(ctx);
        if self.fresh {
            let mut state = self.copper.borrow_mut();
            for _ in 0..PLASMA_WAIT_WALKS {
                state.moveplz();
            }
        }
        self.phase = Phase::PlasmaGate;
        self.tick = 0;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        loop {
            match self.phase {
                Phase::PlasmaGate => {
                    // `PLZ.C:68`: `while(dis_musplus()<0 ...)`.
                    let pass = self.fresh || ctx.music().is_none_or(|music| music.musplus() >= 0);
                    if !pass {
                        return PartStatus::Continuing;
                    }
                    if let Some(music) = ctx.music() {
                        music.setmframe(0);
                    }
                    self.init_plz(ctx);
                    self.phase = Phase::Plasma;
                }
                Phase::Plasma => {
                    if self.plasma_tick(ctx) {
                        self.phase = Phase::PlasmaEndWait;
                    } else {
                        return PartStatus::Continuing;
                    }
                }
                Phase::PlasmaEndWait => {
                    // `PLZ.C:119`: `while(frame_count==0);` — the copper's
                    // increment at the end of this tick ends it.
                    if self.copper.borrow().frame_count == 0 {
                        return PartStatus::Continuing;
                    }
                    {
                        let mut state = self.copper.borrow_mut();
                        state.line_compare = 500; // `set_plzstart(500)`
                        state.cop_plz = false;
                        state.frame_count = 0;
                    }
                    self.set_cube_mode(ctx);
                    self.phase = Phase::CubeGate;
                }
                Phase::CubeGate => {
                    // `VECT.C:116`: `while(dis_musplus()<13 ...)`.
                    let reading = ctx.musplus();
                    if reading < CUBE_GATE {
                        return PartStatus::Continuing;
                    }
                    self.copper.borrow_mut().frame_count = 0;
                    // `VECT.C:116-117`: `while(musplus<13...); frame_count=0;`
                    // and the loop's first `swappage()` are the same frame
                    // period — the page being drawn next tick must already be
                    // the one the copper puts on screen.
                    self.swap_page();
                    self.phase = Phase::Cube;
                }
                Phase::Cube => {
                    return self.cube_tick(ctx);
                }
                Phase::Done => return PartStatus::Finished,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick() {
        // The reference's published row 13 of order 68 first appears at boot
        // 8364 (second3a's trace), i.e. manifest index 8364 - 5723 = 2641
        // (the dump's 0-based numbering; the part's 1-based `tick` is 2642).
        // The player is seeked to order 62 when the part starts; the published
        // words are eleven polls behind the trace's position, so the entry
        // pre-roll is what puts the gate's first pass at the reference tick.
        // This poll is the measurement, not a preference.
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;
        use sr_engine::music::Music;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(62);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        // The engine polls once per tick before the part's frame, so this is
        // part tick 0's poll; `PLZ.C:68-69` then calls `setmframe(0)`, which
        // restarts the row clock the published words advance on.
        music.poll();
        music.setmframe(0);
        let mut tick = 0u32;
        while music.musplus() < CUBE_GATE {
            music.poll();
            tick += 1;
            assert!(tick < 3000, "the gate never passes");
        }
        // The 0-based poll counted here is the dump's manifest index, whose
        // mapping is boot = 5723 + index: 2641 is boot 8364, the trace's
        // first order-68 row 13.
        assert_eq!(
            tick, 2641,
            "the cube gate passes at manifest {tick}, not the reference's \
             boot 8364 (MUSIC_PRE_ROLL = {MUSIC_PRE_ROLL})"
        );
    }
}
