//! TECHNO — the interference, the bars and the troll.
//!
//! `TECHNO/KOE.C` (728 lines) over `KOEA.ASM` (1,065), `KOEB.ASM` (632),
//! `POLYCLIP.ASM` (273) and `READP.C` (72): one part, two screens.
//!
//! 1. **The interference, first pass** (`KOE.C:213-216`; [`interference`]):
//!    the `musplus < -4` gate, `setmframe(0)`, `KOEB.ASM`'s `resetmode13` and
//!    `init_interference`, then 256 frames of `palfader`'s fade over `pal0`.
//!    The display start is the constant 8,020 (`sizefade` stays 0).
//! 2. **The second pass and the bars** (`KOE.C:218-577`): `KOEA.ASM`'s
//!    `do_interference` until `getmframe` reaches 925 ([`interference`]), the
//!    five `flash`es, `mode16`, the four bar wipes on `musrow & 7 == 7`, and
//!    `doit1/2/3`'s 420/840/980 frames of 1-bit boxes ([`bars`]), clipped by
//!    `POLYCLIP.ASM` and filled by `blit16`'s prefix XOR ([`boxes`]).
//! 3. **The troll** (`KOE.C:588-727`; [`troll`]): mode 13h with `inittwk`,
//!    `TROLL.UP`, the order-35 wait, the 300-count pan, the 50-frame ripple
//!    over the 16 `palfade` tables and the final `musplus` wait.
//!
//! The 320×200 screen is EGA 16-colour planar: the port keeps four 64 KiB
//! planes and composes the window ([`Techno::present`]); a row is 40 plane
//! bytes, 80 for the interference and doit3. The 320×400 screen is mode 13h
//! with chain-4 off, which the emulator draws as one byte a pixel from the
//! interleaved planes, so the port keeps that image in the surface (640×400)
//! and lets the `View` read it: the CRTC's plane-byte start ×4 is the pixel
//! origin and the ATC pan is in pixels.
//!
//! Preserved bugs (`bugs.md` §4, §14): `_setpalarea` uploads `2*count + start`
//! bytes ([`Techno::set_pal_area`]); `setborder` returns at once
//! (`KOE.C:68-80`); `flash(-2)` uploads nothing; `patdir` only moves down;
//! `rotate1` leaves the last 64 bytes; the `& 2047` sine cursor is a byte
//! offset; `clipanypoly`'s first edge is the pre-clipped closing edge. No
//! approximations. The timeline and its measurements: `docs/fidelity/techno.md`.

mod bars;
mod boxes;
mod interference;
mod tables;
mod troll;

use sr_assets::Image;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{load_image, read_asset};

const NAME: &str = "techno";

/// `_circle2`: a raw EGA planar picture, 200 rows of 40 bytes a plane.
const CIRCLE2_PATH: &str = "assets/TECHNO/CIRCLE2.EGA";
/// `_circle`: `DDSTARS/PIC.EGA`, 200 rows of three interleaved 40-byte planes.
const CIRCLE_PATH: &str = "assets/DDSTARS/PIC.EGA";
/// `TROLL.UP`: the Origin RLE picture, 320×400, 256 colours.
const TROLL_PATH: &str = "assets/TECHNO/TROLL.UP";

const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
const TROLL_HEIGHT: u16 = 400;
/// One VGA plane's addressable bytes; every plane offset wraps here.
const PLANE_BYTES: usize = 65_536;
/// 40 plane bytes a row: the bars screen (`mode16`'s `13h+256*20`, 20 words).
const PLANE_ROW: usize = 40;
/// 80 plane bytes a row: the interference and doit3 (`init_interference`'s
/// `mov ax,2813h`; doit3's `13h+256*40`).
const PLANE_ROW_WIDE: usize = 80;
/// 160 plane bytes a row: the troll (`13h+256*80`).
const TROLL_ROW: usize = 160;
/// `vbuf = halloc(8192, 1)`, of which 8,000 are used.
const VBUF_BYTES: usize = 8_000;

/// The entry pre-roll: polls that put the published position at MUSIC1 order
/// 18 row 60 — the trace's `second2` reading at 4797 — with `musplus` -4, so
/// the first gate passes on tick 0. A part reached mid-chain does not poll
/// (`music_fresh`); its player is already past this stretch.
const MUSIC_PRE_ROLL: u32 = 253;
/// `KOE.C:213`: `while(!dis_exit() && dis_musplus()<-4);`
const GATE_ONE: i32 = -4;
/// `KOE.C:275`: `while(!dis_exit() && dis_musplus()<-3);`
const GATE_TWO: i32 = -3;
/// `KOEA.ASM:900-904`: the second pass ends at music frame 925.
const INTERFERENCE_END: i32 = 925;

/// Where the part is in the source's straight line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    Gate,
    /// `resetmode13`'s first waitb has returned: clear the DAC.
    ResetClear,
    /// The second waitb has returned: the mode set and the picture.
    ResetMode,
    /// `KOEB.ASM`'s `do_interference`, 256 iterations.
    FirstPass,
    /// `KOEA.ASM`'s `do_interference`, until music frame 925.
    SecondPass,
    /// `flash(32)`, `(64)`, `(192)`, `(256)` (`KOE.C:226-229`).
    Flash(i32),
    /// `while(musplus < -3)`.
    MusplusMinus3,
    /// `while((musrow & 7) != 7)` before the first bar.
    RowWait,
    /// One of the four bar wipes.
    Wipe,
    /// The wipe's `musrow & 7 == 7` wait.
    WipeRowWait,
    /// The wipe's flashes after the wait.
    WipeFlash,
    Doit1,
    Doit2,
    Doit3Prologue,
    Doit3,
    /// The mode 13h / inittwk transition's waits.
    TrollWait1,
    TrollWait2,
    /// `KOE.C:614-636`'s reads and blits (the picture is already decoded).
    TrollBuild,
    TrollPalette,
    TrollOrderWait,
    TrollPan,
    TrollRipple,
    TrollFinalWait,
    Done,
}

pub struct Techno {
    phase: Phase,
    /// The emulator's CPU-bound preamble takes six rendered frames
    /// (4794..4800) over this part's first three ticks: ticks 1 and 2 poll
    /// once more each.
    extra_polls: u8,
    // ---- assets ----
    circle2: Vec<u8>,
    circle: Vec<u8>,
    troll_pixels: Vec<u8>,
    troll_palette: [u8; 768],
    // ---- tables ----
    bar_palette: [u8; 768],
    power0: Vec<i8>,
    flip8: [u8; 256],
    blit: Vec<(u8, u8)>,
    palfade: Vec<u8>,
    // ---- the video memory and the DAC ----
    planes: [Vec<u8>; 4],
    circles: Vec<Vec<u8>>,
    dac: [u8; 768],
    /// CRTC start address in plane bytes (high byte << 8 | low byte).
    display_start: i32,
    /// Plane bytes a displayed row advances.
    stride: usize,
    /// ATC 13h, in displayed pixels.
    pan: u8,
    // ---- the first pass (`KOEB.ASM`'s statics) ----
    palanimc: i32,
    patdir: i32,
    palfader: i32,
    p1_framecount: i32,
    // ---- the second pass (`KOEA.ASM`'s statics) ----
    p2_palanimc: i32,
    p2_patdir: i32,
    p2_framecount: i32,
    p2_sinurot: i32,
    p2_overrot: i32,
    p2_overx: i32,
    p2_overya: i32,
    p2_scrnrot: i32,
    p2_scrnpos: i32,
    p2_scrnposl: i32,
    p2_sinuspower: u8,
    p2_powercnt: u8,
    /// The second pass's first frame shows through the first pass's start
    /// register (the reference's 5058; `techno.md` §4.1).
    p2_first_frame: bool,
    // ---- the flashes and `waitborder` ----
    flash_pal: [i16; 48],
    last_row: i32,
    curpal: i32,
    // ---- the bar wipes ----
    wipe_band: usize,
    wipe_step: i32,
    wipe_zy: i32,
    wipe_zya: i32,
    wipe_flash: u8,
    wipe_pending: Option<i32>,
    // ---- doit1/2/3 and the troll's motion ----
    doit_count: i32,
    doit_first: bool,
    rot: i32,
    rota: i32,
    rot2: i32,
    vm: i32,
    vma: i32,
    plv: i32,
    pl: i32,
    page: usize,
    vbuf: Vec<u8>,
    xpos: i32,
    xposa: i32,
    doit_wx: i32,
    doit_wy: i32,
    repeat: i32,
    ripple: i32,
    ripplep: i32,
    ripple_c: i32,
    // ---- the polygon clipper (`POLYCLIP.ASM`) ----
    clipxy2: [(i16, i16); 32],
    polyxy: [(i16, i16); 32],
    polysides: usize,
}

impl Techno {
    /// A part with nothing loaded: `init` reads the assets.
    pub fn new() -> Techno {
        Techno {
            phase: Phase::Gate,
            extra_polls: 0,
            circle2: Vec::new(),
            circle: Vec::new(),
            troll_pixels: Vec::new(),
            troll_palette: [0; 768],
            bar_palette: tables::bar_palette(),
            power0: tables::power_table().to_vec(),
            flip8: tables::flip8_table(),
            blit: tables::blit_table().to_vec(),
            palfade: Vec::new(),
            planes: std::array::from_fn(|_| vec![0u8; PLANE_BYTES]),
            circles: (0..8).map(|_| vec![0u8; 32_768]).collect(),
            dac: [0; 768],
            display_start: 0,
            stride: PLANE_ROW_WIDE,
            pan: 0,
            palanimc: 0,
            patdir: -3,
            palfader: 0,
            p1_framecount: 0,
            p2_palanimc: 0,
            p2_patdir: 0,
            p2_framecount: 0,
            p2_sinurot: 0,
            p2_overrot: 211,
            p2_overx: 0,
            p2_overya: 0,
            p2_scrnrot: 0,
            p2_scrnpos: 0,
            p2_scrnposl: 0,
            p2_sinuspower: 0,
            p2_powercnt: 0,
            p2_first_frame: false,
            flash_pal: [0; 48],
            last_row: 0,
            curpal: 0,
            wipe_band: 0,
            wipe_step: 0,
            wipe_zy: 0,
            wipe_zya: 0,
            wipe_flash: 0,
            wipe_pending: None,
            doit_count: 0,
            doit_first: true,
            rot: 45,
            rota: 10,
            rot2: 0,
            vm: 0,
            vma: 0,
            plv: 0,
            pl: 1,
            page: 0,
            vbuf: vec![0u8; VBUF_BYTES],
            xpos: 320,
            xposa: 0,
            doit_wx: 160,
            doit_wy: 100,
            repeat: 1,
            ripple: 0,
            ripplep: 8,
            ripple_c: 0,
            clipxy2: [(0, 0); 32],
            polyxy: [(0, 0); 32],
            polysides: 0,
        }
    }

    /// `_setpalarea`'s byte count: `2*count + start`, at byte offset `start`.
    fn set_pal_area(&mut self, src: &[u8], start: usize, count: usize) {
        let bytes = 2 * count + start;
        let end = (start + bytes).min(768);
        let take = end.saturating_sub(start);
        self.dac[start..end].copy_from_slice(&src[..take]);
    }

    /// `outpal`: `src` to the DAC at byte `start`.
    fn upload_dac(&mut self, start: usize, src: &[u8]) {
        let n = src.len().min(768 - start);
        self.dac[start..start + n].copy_from_slice(&src[..n]);
    }

    /// `flash(-1)`: read the 16 DAC entries into `pal1`.
    fn flash_capture(&mut self) {
        for (i, value) in self.flash_pal.iter_mut().enumerate() {
            *value = i16::from(self.dac[i]);
        }
    }

    /// `flash(i)`: `(pal1*j + 63*i) >> 8`, `j = 256 - i`, to DAC 0..15.
    fn flash_apply(&mut self, i: i32) {
        if i == -2 {
            return;
        }
        let j = 256 - i;
        let mut bytes = [0u8; 48];
        for (a, out) in bytes.iter_mut().enumerate() {
            let value = (self.flash_pal[a] as i32 * j + 63 * i) >> 8;
            *out = (value & 0x3F) as u8;
        }
        self.upload_dac(0, &bytes);
    }

    /// Put this tick's DAC on the engine, and the 320×200 window when the
    /// EGA screen is up (the troll's picture is already in the surface).
    fn present(&self, ctx: &mut PartCtx) {
        let mut palette = Palette::new();
        for (entry, rgb) in self.dac.chunks_exact(3).enumerate() {
            palette.set_entry(entry as u8, [rgb[0], rgb[1], rgb[2]]);
        }
        ctx.set_palette(palette);
        if self.stride != TROLL_ROW {
            *ctx.view = crate::parts::mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, None, Some(0));
            self.compose_ega(ctx.surface.pixels_mut());
        }
    }
}

impl Part for Techno {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (TECHNO/KOE.C:213, :275, KOEA.ASM:900) and the run has no music \
             to wait on",
        )?;
        self.circle2 = read_asset(NAME, CIRCLE2_PATH)?;
        self.circle = read_asset(NAME, CIRCLE_PATH)?;
        if self.circle2.len() != 8_000 {
            return Err(PartError::new(
                NAME,
                format!("{CIRCLE2_PATH} is {} bytes, not 8000", self.circle2.len()),
            ));
        }
        if self.circle.len() != 24_000 {
            return Err(PartError::new(
                NAME,
                format!("{CIRCLE_PATH} is {} bytes, not 24000", self.circle.len()),
            ));
        }
        let troll: Image = load_image(NAME, TROLL_PATH)?;
        if (troll.width(), troll.height()) != (SCREEN_WIDTH, TROLL_HEIGHT) {
            return Err(PartError::new(
                NAME,
                format!("{TROLL_PATH} is {}x{}, not 320x400", troll.width(), troll.height()),
            ));
        }
        self.troll_pixels = troll.pixels().to_vec();
        for (entry, rgb) in self.troll_palette.chunks_exact_mut(3).enumerate() {
            rgb.copy_from_slice(&troll.palette.entry(entry as u8));
        }
        self.palfade = tables::troll_fade(&self.troll_palette);
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
            self.extra_polls = 2;
        }
        self.phase = Phase::Gate;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.exit_requested {
            self.phase = Phase::Done;
            return PartStatus::Finished;
        }
        if self.extra_polls > 0 {
            if let Some(music) = ctx.music() {
                music.poll();
            }
            self.extra_polls -= 1;
        }
        match self.phase {
            Phase::Gate => {
                if ctx.musplus() < GATE_ONE {
                    return PartStatus::Continuing;
                }
                if let Some(music) = ctx.music() {
                    music.setmframe(0);
                }
                self.phase = Phase::ResetClear;
            }
            Phase::ResetClear => {
                self.dac = [0; 768];
                self.phase = Phase::ResetMode;
            }
            Phase::ResetMode => {
                // `resetmode13`: mode 13h clears the video memory and reloads
                // the BIOS palette; the second DAC clear follows.
                for plane in &mut self.planes {
                    plane.fill(0);
                }
                self.dac = [0; 768];
                self.pan = 0;
                self.display_start = 0;
                self.init_interference();
                self.phase = Phase::FirstPass;
            }
            Phase::FirstPass => {
                let row = ctx.musrow();
                self.first_pass_iteration(row);
                if self.p1_framecount >= 256 {
                    self.init_interference();
                    self.p2_first_frame = true;
                    self.phase = Phase::SecondPass;
                }
            }
            Phase::SecondPass => {
                let row = ctx.musrow();
                self.second_pass_iteration(row);
                if ctx.getmframe() >= INTERFERENCE_END {
                    self.flash_capture();
                    self.phase = Phase::Flash(32);
                }
            }
            Phase::Flash(i) => {
                self.flash_apply(i);
                self.phase = match i {
                    32 => Phase::Flash(64),
                    64 => Phase::Flash(192),
                    192 => Phase::Flash(256),
                    _ => {
                        self.mode16();
                        Phase::MusplusMinus3
                    }
                };
            }
            Phase::MusplusMinus3 => {
                if ctx.musplus() < GATE_TWO {
                    return PartStatus::Continuing;
                }
                self.phase = Phase::RowWait;
                return self.row_wait(ctx);
            }
            Phase::RowWait => return self.row_wait(ctx),
            Phase::Wipe => {
                self.flush_flash();
                self.wipe_iteration();
            }
            Phase::WipeRowWait => {
                self.flush_flash();
                if ctx.musrow() & 7 != 7 {
                    return PartStatus::Continuing;
                }
                self.flash_capture();
                self.wipe_pending = Some(32);
                self.wipe_flash = 0;
                self.phase = Phase::WipeFlash;
            }
            Phase::WipeFlash => self.wipe_flash_tick(ctx),
            Phase::Doit1 => return self.doit_tick(ctx, 1),
            Phase::Doit2 => return self.doit_tick(ctx, 2),
            Phase::Doit3Prologue => return self.doit3_prologue(ctx),
            Phase::Doit3 => return self.doit3_tick(ctx),
            Phase::TrollWait1 => self.phase = Phase::TrollWait2,
            Phase::TrollWait2 => self.phase = Phase::TrollBuild,
            Phase::TrollBuild => self.phase = Phase::TrollPalette,
            Phase::TrollPalette => {
                let palette = self.troll_palette;
                self.set_pal_area(&palette, 0, 256);
                self.phase = Phase::TrollOrderWait;
                return self.troll_order_wait(ctx);
            }
            Phase::TrollOrderWait => return self.troll_order_wait(ctx),
            Phase::TrollPan => return self.troll_pan(ctx),
            Phase::TrollRipple => return self.troll_ripple(ctx),
            Phase::TrollFinalWait => return self.troll_final_wait(ctx),
            Phase::Done => return PartStatus::Finished,
        }
        self.present(ctx);
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PlayerMusic;
    use crate::parts::test_rig::Rig;

    /// A rig with the real player, `MUSIC1` seeked to order 18. The part sets
    /// no copper and reads no `ctx.frame`, so a step is the engine's poll and
    /// the part's `frame`.
    fn real_rig() -> Rig<PlayerMusic> {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(18);
        Rig::new(320, 200).with(PlayerMusic::new(player, 0))
    }

    /// A real-player run to the end: where each phase begins (frame `n` is the
    /// dump's index `n`), and the view the two screens set.
    #[test]
    fn the_sections_land_on_the_reference_ticks() {
        let mut rig = real_rig();
        let mut part = Techno::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        let mut transitions: Vec<(u32, Phase)> = Vec::new();
        let mut last = part.phase;
        for tick in 0..4200u32 {
            let status = rig.step(&mut part);
            if part.phase != last {
                transitions.push((tick, part.phase));
                last = part.phase;
            }
            // The view changes on frame 3396 (a dump is sized at the tick's
            // start, so the first 320×400 dump is 3397).
            if tick < 3396 {
                assert_eq!((rig.view.width, rig.view.height), (320, 200), "tick {tick}");
            }
            if tick == 3401 {
                assert_eq!((rig.view.width, rig.view.height), (320, 400));
                assert_eq!(rig.view.y_line_offset, Some(640), "a 160-byte plane row ×4");
                assert_eq!(rig.view.start_addr, 0, "the picture slides in from x = 0");
            }
            if status == PartStatus::Finished {
                break;
            }
        }
        use Phase::*;
        assert_eq!(
            transitions,
            vec![
                (0, ResetClear),
                (1, ResetMode),
                (2, FirstPass),
                (258, SecondPass),
                (1246, Flash(32)),
                (1247, Flash(64)),
                (1248, Flash(192)),
                (1249, Flash(256)),
                (1250, MusplusMinus3),
                (1295, RowWait),
                (1303, Wipe),
                (1323, WipeRowWait),
                (1335, WipeFlash),
                (1339, Wipe),
                (1359, WipeRowWait),
                (1367, WipeFlash),
                (1371, Wipe),
                (1391, WipeRowWait),
                (1400, WipeFlash),
                (1404, Wipe),
                (1424, WipeRowWait),
                (1432, WipeFlash),
                (1436, Doit1),
                (1857, Doit2),
                (2698, Doit3Prologue),
                (2699, Doit3),
                (3396, TrollWait1),
                (3397, TrollWait2),
                (3398, TrollBuild),
                (3399, TrollPalette),
                (3400, TrollOrderWait),
                (3445, TrollPan),
                (3498, TrollRipple),
                (3547, TrollFinalWait),
                (3967, Done),
            ]
        );
    }

    #[test]
    fn the_palette_upload_is_two_bytes_a_colour_and_truncates_at_512() {
        let mut t = Techno::new();
        t.dac = [7; 768];
        let table = vec![9u8; 768];
        t.set_pal_area(&table, 0, 256);
        assert_eq!(&t.dac[..512], &[9u8; 512], "2*256 + 0 bytes");
        assert_eq!(&t.dac[512..], &[7u8; 256], "entries 171.. keep what they had");
        t.dac = [7; 768];
        t.set_pal_area(&table, 4, 2);
        assert_eq!(&t.dac[4..12], &[9u8; 8], "2*2 + 4 bytes");
        assert_eq!(t.dac[3], 7);
        assert_eq!(t.dac[12], 7);
    }

    #[test]
    fn flash_minus_two_uploads_nothing() {
        let mut t = Techno::new();
        t.dac = [7u8; 768];
        t.flash_pal = [11i16; 48];
        t.flash_apply(-2);
        assert_eq!(t.dac, [7u8; 768], "flash(-2) uploads nothing");
        assert_eq!(t.flash_pal, [11i16; 48], "and captures nothing");
    }

    #[test]
    fn the_entry_pre_roll_lands_the_reference_position_at_tick_zero() {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;
        use sr_engine::music::Music;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(18);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        let position = music.position();
        assert_eq!((position.order, position.row), (18, 60));
        assert_eq!(music.musplus(), -4, "the first gate (`<-4`) passes");
        // The engine's tick-0 poll leaves the published row where it was.
        music.poll();
        let position = music.position();
        assert_eq!((position.order, position.row), (18, 60));
        assert_eq!(music.musplus(), -4);
    }
}
