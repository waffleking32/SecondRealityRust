//! RAYSCRL — the raytraced mirror-ball scroll.
//!
//! `WATER/DEMO.PAS` (190 lines) over `WATER/ROUTINES.ASM` (`Putrouts1`, the
//! colour-keyed column blitter) and the linked `BKG.CLX` (`BKR.PAS`'s
//! `_tausta`), `MIEKKA.SCI` (`MIEK.PAS`'s `_miekka`) and `WAT1-3.DAT`
//! (`T1-3.PAS`), all read in their original formats. A tick is one `waitr`
//! (`:13-20`):
//!
//! ```text
//! ticks 0-6        :98-104   the init wait for musplus >= 0, storing the order as co
//! tick 7           :107-116  the loads, the black DAC and the BKG.CLX blit
//! ticks 8-134      :117-132  127 fade-in iterations: upload and step on odd y, scr(sss)
//! ticks 135-653    :135-143  the spin until the order leaves co and the row is >= 16
//! tick 654 on      :155-189  the main loop: scr(sss), the font shifted and refilled
//!                            every third tick, the fade-out on musplus == -11 exactly
//! ```
//!
//! `Putrouts1` (`ROUTINES.ASM:12-40`) writes each column's font byte at every
//! offset of one of the three lists, and where the byte is 0 the background
//! pixel instead: a colour key, not MNTSCRL's add. The shift and refill are in
//! the main loop (`:183-185`), after the `scr(2)` that drew with the old font.
//! `MIEKKA.SCI`'s palette is the part's, not `BKG.CLX`'s.
//!
//! Tick 0 is boot 12962, where the reference publishes 88:0; a mapping, not an
//! anchor (`docs/fidelity/rayscrl.md` §2), so `init` makes the music's head
//! start itself. The release's three gates act seven ticks after the tree's
//! ([`GATE_CONFIRM_TICKS`], A18).
//!
//! Preserved bugs (`bugs.md` §19): the fade loops' `pf := 0 to 3` and the
//! `pal[768]` alias ([`ramp`]); the refill one band down, the `move` one byte
//! past `fbuf` and the `scp < 390` stop ([`Rayscrl::shift_and_fill`]).

use sr_assets::poslist::PosList;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{load_image, mode_view, read_asset};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "rayscrl";

// ------------------------------------------------------------- the screen --

/// `DEMO.PAS:91` `mov ax,$13`: 320×200, one byte a pixel, rows packed.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// The displayed picture: `:113`'s 64,000-byte blit.
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;

// ------------------------------------------------------------- the assets --

/// `DEMO.PAS:112-113` `_tausta`: 320x200, picture at +778.
const BKG_PATH: &str = "assets/WATER/BKG.CLX";
const BKG_WIDTH: usize = 320;
const BKG_HEIGHT: usize = 200;
/// `DEMO.PAS:108-109` `_miekka`: it declares 400x250 and carries 35 rows.
const MIEKKA_PATH: &str = "assets/WATER/MIEKKA.SCI";
const MIEKKA_WIDTH: usize = 400;
const MIEKKA_ROWS: usize = 35;
/// `:109` `move(...,font,400*34)`: the bytes the part loads.
const FONT_LOADED: usize = MIEKKA_WIDTH * 34;
/// `:38` `font : array[0..400*35] of byte`: 14,001 bytes.
const FONT_LEN: usize = MIEKKA_WIDTH * MIEKKA_ROWS + 1;

/// `T1-3.PAS` link the three lists; `DEMO.PAS:51-80` picks one.
const POS_PATHS: [&str; 3] = [
    "assets/WATER/WAT1.DAT",
    "assets/WATER/WAT2.DAT",
    "assets/WATER/WAT3.DAT",
];
/// `ROUTINES.ASM:21` `mov dx,158*34`: the columns the blitter walks.
const FBUF_COLUMNS: usize = 158 * 34;
/// `:39` `fbuf : array[0..158*34] of byte`: 5,373 bytes, plus the byte the
/// `move` reads past it (`scp`'s low byte).
const FBUF_LEN: usize = FBUF_COLUMNS + 1;
/// `:184` `for x := 0 to 33`: the bands the refill writes.
const FONT_BANDS: usize = 34;
/// `:184` `158 + x*158`: the refill's stride, one band down.
const FBUF_STRIDE: usize = 158;
/// `:185` `if scp < 390 then inc(scp)`.
const SCP_MAX: u16 = 390;

// ------------------------------------------------------------ the palette --

/// `:117` `for y := 0 to 63*2`: 127 iterations.
const FADE_IN_STEPS: u32 = 127;
/// `:170` `if fp = 64 then quit := 1 else inc(fp)`: 65 ticks.
const FADE_OUT_STEPS: u32 = 65;
/// `:207` `for x := 0 to 255`.
const PALETTE_ENTRIES: usize = 256;
/// `:37` `tmppal,pal : array[0..768]`: 769 bytes each ([`ramp`]).
const PALETTE_LEN: usize = 769;

// ---------------------------------------------------------------- the music --

/// `:162` `cmp dx,-11 / jnz @a`: the fade begins at exactly -11.
const CUE: i32 = -11;
/// `:139-141` `cmp cx,CO / je @a / cmp bx,16 / jl @a`.
const STAGE2_ROW: u8 = 16;
/// The release's gates act on the tick after their condition has read true
/// on this many consecutive ticks; the tree acts on the first (D24, A18;
/// measured in `docs/fidelity/rayscrl.md` §4).
const GATE_CONFIRM_TICKS: u8 = 7;
/// Polls `init` makes on a fresh start: after the shell's seek to order 86
/// the player publishes 88:0 on its 269th poll, which is the engine's tick-0
/// poll. Seeking straight to 88 would leave `zplus` 0 and the init wait
/// would never pass.
const MUSIC_PRE_ROLL: u32 = 268;

// --------------------------------------------------------------- the part --

/// Where in `DEMO.PAS`'s flow the part is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `:98-104`: the init wait for `musplus >= 0`, confirmed.
    Init,
    /// `:107-116`: the loads and the blit, on the tick after the confirm.
    Setup,
    /// `:120-131`: fade-in iteration `y` (0 through 126).
    FadeIn { y: u32 },
    /// `:135-143`: the order-change/row gate, confirmed.
    Stage2,
    /// `:155-189`: the main loop.
    Main,
    /// The part has ended and keeps its last frame.
    Done,
}

/// RAYSCRL: the raytraced picture, its font strip and the three position lists.
pub struct Rayscrl {
    background: Vec<u8>,
    font: Vec<u8>,
    fbuf: Vec<u8>,
    lists: Vec<PosList>,
    /// `:37` `pal`, the fade's value array.
    pal: Vec<u8>,
    /// `:37` `tmppal`, the fade's target array.
    tmppal: Vec<u8>,
    /// `:84` `co`: the order the init wait captured.
    co: u16,
    /// `:40` `scp`.
    scp: u16,
    /// `:40` `sss`.
    sss: u8,
    /// `:87` `fp`: the fade-out's step.
    fp: u16,
    /// `:85` `fadeout`: set by the cue and never cleared.
    fadeout: bool,
    /// `:86` `quit`: set on the fade-out's last tick.
    quit: bool,
    /// Consecutive ticks the current gate's condition has held.
    confirm: u8,
    /// The cue is confirmed; the fade acts next tick.
    cue_armed: bool,
    /// The gates' confirmation: [`GATE_CONFIRM_TICKS`], or 0 for the tree's
    /// immediate condition (A18's counterfactual).
    confirm_ticks: u8,
    phase: Phase,
}

impl Rayscrl {
    /// A part with nothing loaded: `init` reads the three assets.
    pub fn new() -> Rayscrl {
        Rayscrl {
            background: Vec::new(),
            font: Vec::new(),
            fbuf: Vec::new(),
            lists: Vec::new(),
            pal: vec![0; PALETTE_LEN],
            tmppal: vec![0; PALETTE_LEN],
            co: 0,
            scp: 0,
            sss: 0,
            fp: 0,
            fadeout: false,
            quit: false,
            confirm: 0,
            cue_armed: false,
            confirm_ticks: GATE_CONFIRM_TICKS,
            phase: Phase::Init,
        }
    }

    /// Whether `condition` has now held for `confirm_ticks` consecutive ticks.
    fn confirm(&mut self, condition: bool) -> bool {
        if self.confirm_ticks == 0 {
            return condition;
        }
        self.confirm = if condition { self.confirm.saturating_add(1) } else { 0 };
        self.confirm == self.confirm_ticks
    }

    /// `:120-131`: fade-in iteration `y`: on odd `y` the upload and step, then
    /// one `scr`, `sss` cycling.
    fn fade_in_tick(&mut self, ctx: &mut PartCtx, y: u32) {
        if y % 2 == 1 {
            ctx.set_palette(ramp(&mut self.pal, &self.tmppal, Direction::Up));
        }
        self.scr(ctx, self.sss as usize);
        self.sss = if self.sss == 2 { 0 } else { self.sss + 1 };
    }

    /// `:155-189`: one main-loop tick.
    fn main_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if !self.fadeout {
            if self.cue_armed {
                self.fadeout = true;
            } else if self.confirm(ctx.musplus() == CUE) {
                self.cue_armed = true;
            }
        }
        if self.fadeout {
            // `:167-178`.
            if self.fp == (FADE_OUT_STEPS - 1) as u16 {
                self.quit = true;
            } else {
                self.fp += 1;
            }
            ctx.set_palette(ramp(&mut self.pal, &self.tmppal, Direction::Down));
        }

        // `:179-185`.
        self.scr(ctx, self.sss as usize);
        if self.sss == 2 {
            self.sss = 0;
            self.shift_and_fill();
        } else {
            self.sss += 1;
        }

        // `:189` `until dis_exit or (quit = 1)`.
        if self.quit || ctx.exit_requested {
            self.phase = Phase::Done;
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }

    /// `:51-80` `scr(pos)`: one list's columns into video memory, a zero font
    /// byte replaying the background pixel (`ROUTINES.ASM:28-32`).
    fn scr(&mut self, ctx: &mut PartCtx, which: usize) {
        let memory = ctx.surface.pixels_mut();
        let list = &self.lists[which];
        for column in 0..FBUF_COLUMNS {
            let value = self.fbuf[column];
            for &offset in list.column(column) {
                let at = offset as usize;
                memory[at] = if value == 0 { self.background[at] } else { value };
            }
        }
    }

    /// `:183-185`: `move(fbuf[1],fbuf,sizeof(fbuf))`, which reads one byte
    /// past the array (in the original `scp`'s low byte), the refill
    /// `fbuf[158+x*158] := font[x*400+scp]` one band down, so index 0 never
    /// gets a sample, and `scp` toward 390, ten columns short of the source.
    fn shift_and_fill(&mut self) {
        self.fbuf[FBUF_LEN] = (self.scp & 0xff) as u8;
        self.fbuf.copy_within(1..=FBUF_LEN, 0);
        for x in 0..FONT_BANDS {
            self.fbuf[FBUF_STRIDE + x * FBUF_STRIDE] =
                self.font[x * MIEKKA_WIDTH + self.scp as usize];
        }
        if self.scp < SCP_MAX {
            self.scp += 1;
        }
    }
}

/// Parse one POS list, `bytes` as read from `path`, with every offset inside
/// the picture `scr` indexes and the blitter's column count.
fn pos_list(path: &str, bytes: &[u8]) -> Result<PosList, PartError> {
    let list =
        PosList::parse(bytes).map_err(|error| PartError::new(NAME, format!("{path}: {error}")))?;
    for (column, offsets) in list.columns.iter().enumerate() {
        if let Some(&offset) = offsets.iter().find(|&&offset| offset as usize >= VIDEO_BYTES) {
            return Err(PartError::new(
                NAME,
                format!(
                    "{path}: column {column} holds offset {offset}, outside the \
                     {VIDEO_BYTES}-byte picture"
                ),
            ));
        }
    }
    if list.columns.len() != FBUF_COLUMNS {
        return Err(PartError::new(
            NAME,
            format!(
                "{path} holds {} columns; the blitter walks {FBUF_COLUMNS} (158*34, \
                 WATER/ROUTINES.ASM:21)",
                list.columns.len()
            ),
        ));
    }
    Ok(list)
}

impl Part for Rayscrl {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `:98-116`'s loads. The source's wait and loads run before its first
    /// `waitr`, so `init` reads the assets and `frame` runs the wait.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (WATER/DEMO.PAS:98-104, :135-143, :158-166) and the run has no music \
             to wait on",
        )?;
        // The head start belongs to the entry run alone: a part reached
        // mid-chain is handed a player already through this stretch.
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
        }

        // `:108-109`: the palette and the first 400*34 pixels of MIEKKA.SCI.
        let miekka = load_image(NAME, MIEKKA_PATH)?;
        if (miekka.width() as usize, miekka.height() as usize) != (MIEKKA_WIDTH, MIEKKA_ROWS) {
            return Err(PartError::new(
                NAME,
                format!(
                    "{MIEKKA_PATH} is {}x{}; the part reads {MIEKKA_WIDTH}x{MIEKKA_ROWS}",
                    miekka.width(),
                    miekka.height()
                ),
            ));
        }
        // `:115-116` `move(pal,tmppal,768); fillchar(pal,768,#0)`; both 769th
        // bytes stay 0 (`ramp` reads `tmppal[768]` as its alias, `pal[0]`).
        let mut tmppal = vec![0; PALETTE_LEN];
        for index in 0..PALETTE_ENTRIES {
            tmppal[index * 3..index * 3 + 3].copy_from_slice(&miekka.palette.entry(index as u8));
        }
        let mut font = vec![0; FONT_LEN];
        font[..FONT_LOADED].copy_from_slice(&miekka.pixels()[..FONT_LOADED]);

        // `:112-113` `_tausta`: the picture at +778 rebased to +0.
        let bkg = load_image(NAME, BKG_PATH)?;
        if (bkg.width() as usize, bkg.height() as usize) != (BKG_WIDTH, BKG_HEIGHT) {
            return Err(PartError::new(
                NAME,
                format!(
                    "{BKG_PATH} is {}x{}; the part reads {BKG_WIDTH}x{BKG_HEIGHT}",
                    bkg.width(),
                    bkg.height()
                ),
            ));
        }

        let lists = POS_PATHS
            .iter()
            .map(|rel| pos_list(rel, &read_asset(NAME, rel)?))
            .collect::<Result<Vec<_>, _>>()?;

        *self = Rayscrl {
            background: bkg.pixels()[..VIDEO_BYTES].to_vec(),
            font,
            // `:39` `fbuf`, zeroed at `:107`, plus the byte the `move` reads.
            fbuf: vec![0; FBUF_LEN + 1],
            lists,
            tmppal,
            ..Rayscrl::new()
        };

        // `:90-92`: mode 13h and the display state a mode set leaves.
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(SCREEN_WIDTH), Some(0));
        Ok(())
    }

    /// One tick of `DEMO.PAS:98-189`; the waits honour the exit notice the
    /// source never checks there.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Main => self.main_tick(ctx),
            _ if ctx.exit_requested => {
                self.phase = Phase::Done;
                PartStatus::Finished
            }
            Phase::Init => {
                if self.confirm(ctx.musplus() >= 0) {
                    self.phase = Phase::Setup;
                }
                PartStatus::Continuing
            }
            Phase::Setup => {
                // `:103` `mov co,cx`: the published order at the gate's leave.
                self.co = u16::from(ctx.music().map_or(0, |music| music.position().order));
                // `:111-113`: the DAC black and the picture blitted.
                ctx.surface.pixels_mut()[..VIDEO_BYTES].copy_from_slice(&self.background);
                ctx.set_palette(Palette::new());
                self.phase = Phase::FadeIn { y: 0 };
                PartStatus::Continuing
            }
            Phase::FadeIn { y } => {
                self.fade_in_tick(ctx, y);
                if y + 1 == FADE_IN_STEPS {
                    self.confirm = 0;
                    self.phase = Phase::Stage2;
                } else {
                    self.phase = Phase::FadeIn { y: y + 1 };
                }
                PartStatus::Continuing
            }
            Phase::Stage2 => {
                // `:135-143`, no `waitb`: the port polls once a tick.
                let (order, row) = ctx.music().map_or((0, 0), |music| {
                    let position = music.position();
                    (position.order, position.row)
                });
                if self.confirm(u16::from(order) != self.co && row >= STAGE2_ROW) {
                    // `:149-154`: the resets and `tmppal` zeroed; the main
                    // loop's first `waitr` is the next tick.
                    self.sss = 0;
                    self.scp = 0;
                    self.fp = 0;
                    self.fadeout = false;
                    self.quit = false;
                    self.confirm = 0;
                    self.tmppal[..PALETTE_ENTRIES * 3].fill(0);
                    self.phase = Phase::Main;
                }
                PartStatus::Continuing
            }
        }
    }
}

/// Which way a fade step moves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

/// One fade step (`:122-126`, `:171-175`): each entry is uploaded from `pal`,
/// then its channels stepped one toward `tmppal` (or toward zero).
///
/// The `pf` loop runs `0 to 3`, so `pal[x*3+3]`, the next entry's red, is
/// stepped too, after entry `x` was uploaded: every red but entry 0's moves
/// twice a pass and shows one step ahead. At `x = 255` it touches index 768;
/// in the original `pal[768]` is `font[0]` and `tmppal[768]` is `pal[0]`, so
/// the port keeps the byte and reads the alias. Entry 0 is black in the real
/// palette, so the font is untouched.
fn ramp(pal: &mut [u8], tmppal: &[u8], direction: Direction) -> Palette {
    let mut dac = Palette::new();
    for entry in 0..PALETTE_ENTRIES {
        dac.set_entry(entry as u8, [pal[entry * 3], pal[entry * 3 + 1], pal[entry * 3 + 2]]);
        for pf in 0..=3 {
            let at = entry * 3 + pf;
            let target = if at == PALETTE_LEN - 1 { pal[0] } else { tmppal[at] };
            match direction {
                Direction::Up if pal[at] < target => pal[at] += 1,
                Direction::Down if pal[at] > target => pal[at] -= 1,
                _ => {}
            }
        }
    }
    dac
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::PlayerMusic;
    use crate::parts::test_rig::{
        differing_pixels, reference, repo, Reading, Rig, RigMusic, ScriptedMusic,
    };
    use sr_engine::dac_expand;
    use sr_engine::music::Music;

    /// The reference's tick 0: boot 12962 (the module doc's mapping).
    const REFERENCE_FIRST_INDEX: u32 = 12_962;
    /// The RIX3 palette's offset in a file, after the 10-byte header.
    const RIX3_PALETTE: usize = 10;
    /// The tick the port's published `musplus` first stands at -11 (boot 14943).
    const CUE_FIRST: u32 = 1_981;
    /// The tick the fade's first upload lands on: boot 14950.
    const FADE_FIRST: u32 = 1_988;

    fn tick_of(index: u32) -> u32 {
        index - REFERENCE_FIRST_INDEX
    }

    /// A player scripted one `(order, row, musplus)` reading a tick; a tick
    /// past the script reads 88:0 with `musplus` 0.
    fn scripted(script: Vec<(u8, u8, i32)>) -> ScriptedMusic {
        let reading = |(order, row, musplus)| Reading { order, row, musplus, sync: 0 };
        ScriptedMusic::readings(script.into_iter().map(reading).collect())
            .past_end(reading((88, 0, 0)))
            .never_syncs("`DEMO.PAS` never syncs: it calls `dis_musplus` and `dis_muscode`")
    }

    /// The init passes (`musplus` 0) and the stage-2 gate never sees the
    /// order change.
    fn quiet(last_tick: u32) -> ScriptedMusic {
        scripted(vec![(88, 0, 0); last_tick as usize + 1])
    }

    /// A script whose stage 2 passes at `stage_tick` (order 89, row 16) and
    /// whose `musplus` is `cue` from `cue_tick`.
    fn through_stage_two(
        stage_tick: u32,
        cue: Option<(u32, i32)>,
        last_tick: u32,
    ) -> ScriptedMusic {
        let mut script = vec![(88, 0, 0); last_tick as usize + 1];
        for entry in script.iter_mut().skip(stage_tick as usize) {
            *entry = (89, 16, 0);
        }
        if let Some((cue_tick, musplus)) = cue {
            for entry in script.iter_mut().skip(cue_tick as usize) {
                *entry = (91, 53, musplus);
            }
        }
        scripted(script)
    }

    fn rig<M: RigMusic>(music: M) -> Rig<M> {
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(music)
    }

    fn started_with<M: RigMusic>(music: M) -> (Rig<M>, Rayscrl) {
        let mut rig = rig(music);
        let mut part = Rayscrl::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Rayscrl) {
        started_with(quiet(0))
    }

    /// A part with the real player: `MUSIC1` seeked to order 86, as a
    /// `--part rayscrl` run hands it.
    fn started_real() -> (Rig<PlayerMusic>, Rayscrl) {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(86);
        started_with(PlayerMusic::new(player, 0))
    }

    /// Step the part until the rig's tick is `tick`, asserting it keeps going.
    fn run_to<M: RigMusic>(rig: &mut Rig<M>, part: &mut Rayscrl, tick: u32) {
        while rig.tick < tick {
            let at = rig.tick;
            assert_eq!(rig.step(part), PartStatus::Continuing, "tick {at}");
        }
    }

    // --- the cue and the exit ------------------------------------------------

    #[test]
    fn the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks() {
        // -11 exactly, confirmed seven ticks, then 64 increments and the
        // 65th tick's `quit`.
        let (mut rig, mut part) =
            started_with(through_stage_two(700, Some((CUE_FIRST, CUE)), FADE_FIRST + 200));
        run_to(&mut rig, &mut part, FADE_FIRST);
        assert_eq!(part.phase, Phase::Main);
        assert!(!part.fadeout, "the fade acts on the tick after the confirm");

        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert!(part.fadeout, "the eighth consecutive -11 starts the fade");
        assert_eq!(part.fp, 1);
        // The first upload is the fade-in's stored `pal` (its 63s), which the
        // screen has not shown since the fade-in's last upload (62s).
        assert_eq!(rig.palette.entry(39)[2], 63);

        run_to(&mut rig, &mut part, FADE_FIRST + FADE_OUT_STEPS - 1);
        assert_eq!(part.fp, 64);
        assert_eq!(rig.step(&mut part), PartStatus::Finished);
        assert!(part.quit);
        assert_eq!(part.phase, Phase::Done);
        assert!(
            (0..256).all(|index| rig.palette.entry(index as u8) == [0, 0, 0]),
            "the last fade tick uploads black"
        );
        assert_eq!(rig.step(&mut part), PartStatus::Finished);
    }

    #[test]
    fn minus_ten_and_minus_twelve_never_trigger_the_cue() {
        for musplus in [-12, -10] {
            let (mut rig, mut part) =
                started_with(through_stage_two(700, Some((CUE_FIRST, musplus)), 2_500));
            run_to(&mut rig, &mut part, FADE_FIRST + 100);
            assert!(!part.fadeout, "{musplus} is not the cue");
            assert_eq!(part.phase, Phase::Main);
            assert_ne!(
                rig.surface.pixels()[..VIDEO_BYTES],
                part.background[..],
                "the scroller keeps drawing"
            );
        }
    }

    #[test]
    fn exit_requested_finishes_on_that_tick_in_every_phase() {
        // The init wait.
        let (mut rig, mut part) = started_with(quiet(10));
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished, "the init wait");
        // The fade.
        let (mut rig, mut part) = started_with(quiet(10));
        run_to(&mut rig, &mut part, 20);
        assert!(matches!(part.phase, Phase::FadeIn { .. }));
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished, "the fade");
        // Stage 2, the source's one true spin, which never checks `dis_exit`.
        let (mut rig, mut part) = started_with(quiet(1_000));
        run_to(&mut rig, &mut part, 200);
        assert_eq!(part.phase, Phase::Stage2);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished, "stage 2");
        assert_eq!(part.phase, Phase::Done);
        // The main loop.
        let (mut rig, mut part) = started_with(through_stage_two(700, None, 900));
        run_to(&mut rig, &mut part, 720);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(rig.step_exit(&mut part), PartStatus::Finished, "the main loop");
    }

    // --- the assets ----------------------------------------------------------

    #[test]
    fn the_assets_are_read_the_way_the_source_reads_them() {
        let (_, part) = started();

        // `:108-109`: MIEKKA.SCI's palette is the part's; its first 400*34
        // pixel bytes are the font strip.
        let miekka = read_asset(NAME, MIEKKA_PATH).unwrap();
        assert_eq!(miekka.len(), 14_778);
        assert_eq!(
            &part.tmppal[..PALETTE_ENTRIES * 3],
            &miekka[RIX3_PALETTE..RIX3_PALETTE + PALETTE_ENTRIES * 3],
            "MIEKKA.SCI's palette at +10"
        );
        // MIEKKA's palette wins over BKG.CLX's: 64 of the 256 entries differ.
        let bkg = read_asset(NAME, BKG_PATH).unwrap();
        let entry = |file: &[u8], index: usize| file[RIX3_PALETTE + index * 3..][..3].to_vec();
        let differing_entries = (0..PALETTE_ENTRIES)
            .filter(|&index| entry(&miekka, index) != entry(&bkg, index))
            .count();
        assert_eq!(differing_entries, 64);
        assert!(part.pal.iter().all(|byte| *byte == 0), "`:116` fills pal with zero");
        assert_eq!(part.font.len(), FONT_LEN);
        assert_eq!(&part.font[..FONT_LOADED], &miekka[778..778 + FONT_LOADED]);
        assert!(part.font[FONT_LOADED..].iter().all(|byte| *byte == 0));

        // `:112-113`: the picture at +778 rebased to +0.
        assert_eq!(bkg.len(), 64_778);
        assert_eq!(part.background, bkg[778..778 + VIDEO_BYTES]);

        // `:51-80`: three lists of 158*34 columns (`reference-indices.md`
        // §15's asset table), the largest destination inside the picture.
        let mut per_list = Vec::new();
        let mut max_offset = 0u16;
        for (path, list) in POS_PATHS.iter().zip(&part.lists) {
            assert_eq!(list.columns.len(), FBUF_COLUMNS, "{path}");
            per_list.push((
                crate::assets::read(path).unwrap().len(),
                list.columns.iter().map(Vec::len).sum::<usize>(),
            ));
            max_offset = max_offset.max(list.columns.iter().flatten().copied().max().unwrap());
        }
        assert_eq!(per_list, [(23_096, 6_176), (23_034, 6_145), (27_818, 8_537)]);
        assert_eq!(max_offset, 63_999);
    }

    #[test]
    fn an_offset_past_the_picture_is_refused_not_indexed() {
        // A full column count, so the offset bound answers, not the count check.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes()); // column 0: one offset
        bytes.extend_from_slice(&65_000u16.to_le_bytes());
        for _ in 1..FBUF_COLUMNS {
            bytes.extend_from_slice(&0u16.to_le_bytes()); // an empty column
        }
        let error = pos_list("POS.DAT", &bytes).expect_err("65,000 is outside the picture");
        for part in ["POS.DAT", "column 0", "65000"] {
            assert!(error.message.contains(part), "{part}: {}", error.message);
        }
    }

    #[test]
    fn the_trees_miekka_object_carries_the_files_bytes() {
        // `rayscrl.md` §6: the OMF object's LEDATA records are the file.
        let obj = std::fs::read(repo("upstream/SecondReality/WATER/MIEKKA.OBJ")).unwrap();
        let embedded = sr_assets::omf::extract_ledata(&obj).unwrap();
        assert_eq!(embedded, read_asset(NAME, MIEKKA_PATH).unwrap());
    }

    #[test]
    fn the_music_pre_roll_puts_the_reference_state_at_tick_zero() {
        // `play_song(86)` walks the `254` at order-list entry 87 into order 88
        // with `zplus` 2; the 269th poll publishes 88:0.
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(86);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        assert_eq!(music.position().order, 86, "268 polls is still order 86");
        // The engine's tick-0 poll.
        music.poll();
        assert_eq!((music.position().order, music.position().row), (88, 0));
        assert_eq!(music.musplus(), 0, "`musplus >= 0` passes at tick 0");
        // Row 1 arrives on the row's eighth poll, the reference's 12970.
        for _ in 0..8 {
            music.poll();
        }
        assert_eq!((music.position().order, music.position().row), (88, 1));
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        let mut rig = rig(quiet(0));
        let mut part = Rayscrl::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player().polls, 0, "a mid-chain start polls nothing");
    }

    // --- the fade ------------------------------------------------------------

    #[test]
    fn the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero() {
        let (mut rig, mut part) = started();
        // The blit lands on the tick after the init's confirmation.
        run_to(&mut rig, &mut part, 7);
        assert_eq!(part.phase, Phase::Setup);
        run_to(&mut rig, &mut part, 8);
        assert_eq!(part.phase, Phase::FadeIn { y: 0 });
        // The last iteration is y=126 at tick 134; stage 2 starts at 135.
        run_to(&mut rig, &mut part, 134);
        assert_eq!(part.phase, Phase::FadeIn { y: 126 });
        run_to(&mut rig, &mut part, 135);
        assert_eq!(part.phase, Phase::Stage2);

        // Boot 13000 is tick 38, the hold of the k=14 upload made at y=29
        // (rediff's 13000 row is exact). The stored array is one step past
        // the upload: `:125`'s loop uploads before it steps.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, tick_of(13_000) + 1);
        assert_eq!(part.pal[39 * 3 + 2], 15);
        assert_eq!(rig.palette.entry(39)[2], 14);

        // The same shortfall at the fade's end: the reference's 13500 palette
        // (its manifest) is MIEKKA's with exactly seven blue channels at 62
        // where the target is 63.
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(repo("reference/parts/rayscrl/manifest.json")).unwrap(),
        )
        .unwrap();
        let held = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["index"] == 13_500)
            .unwrap();
        let miekka = read_asset(NAME, MIEKKA_PATH).unwrap();
        let mut short = Vec::new();
        for (index, rgb) in held["palette"].as_object().unwrap() {
            let index: usize = index.parse().unwrap();
            let theirs: Vec<u64> =
                rgb.as_array().unwrap().iter().map(|c| c.as_u64().unwrap()).collect();
            let ours: Vec<u64> =
                miekka[RIX3_PALETTE + index * 3..][..3].iter().map(|&c| u64::from(c)).collect();
            assert_eq!(theirs[..2], ours[..2], "entry {index}'s red and green");
            if theirs[2] != ours[2] {
                assert_eq!((ours[2], theirs[2]), (63, 62), "entry {index}");
                short.push(index);
            }
        }
        assert_eq!(short.len(), 7, "seven blues stop one step short");
    }

    #[test]
    fn the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array() {
        // `:125` / `:174` `for pf := 0 to 3`: index x*3+3 is the next entry's
        // red, so every red but entry 0's is stepped twice a pass.
        let mut pal = vec![0u8; PALETTE_LEN];
        let mut tmppal = vec![0u8; PALETTE_LEN];
        tmppal[3..30].fill(63);
        ramp(&mut pal, &tmppal, Direction::Up);
        // Entry 1's red by passes 0 and 1, entry 2's by 1 and 2.
        assert_eq!(pal[3..7], [2, 1, 1, 2]);

        // `x = 255`, `pf = 3` touches index 768 against the alias `pal[0]`;
        // a synthetic `pal[0]` makes it observable.
        let mut pal = vec![0u8; PALETTE_LEN];
        let mut tmppal = vec![0u8; PALETTE_LEN];
        pal[0] = 5;
        tmppal[0] = 5;
        ramp(&mut pal, &tmppal, Direction::Up);
        assert_eq!(pal[768], 1, "the byte past the array is stepped");
        // The real palette leaves it alone.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 136);
        assert_eq!((part.pal[0], part.pal[768], part.font[0]), (0, 0, 0));
    }

    // --- the blitter ---------------------------------------------------------

    /// A part with synthetic assets: three lists whose columns hold a handful
    /// of offsets, a flat background and a zero font.
    fn synthetic() -> Rayscrl {
        let mut columns = vec![Vec::new(); FBUF_COLUMNS];
        columns[0] = vec![10, 20];
        columns[2] = vec![40];
        columns[3] = vec![50];
        let mut part = Rayscrl::new();
        part.background = vec![7; VIDEO_BYTES];
        part.fbuf = vec![0; FBUF_LEN + 1];
        part.font = vec![0; FONT_LEN];
        part.lists = (0..3).map(|_| PosList { columns: columns.clone() }).collect();
        part
    }

    #[test]
    fn the_blitter_replays_the_background_where_the_font_byte_is_zero() {
        // `ROUTINES.ASM:28-32`: a colour key, with no +128 bias.
        let mut rig = rig(quiet(0));
        let mut part = synthetic();
        part.fbuf[0] = 9;
        part.scr(&mut rig.ctx(0), 0);
        let pixels = |rig: &Rig| [10, 20, 30, 40, 50].map(|at| rig.surface.pixels()[at]);
        assert_eq!(pixels(&rig), [9, 9, 0, 7, 7], "font 0 replays the background");
        part.fbuf[3] = 128;
        part.scr(&mut rig.ctx(0), 0);
        assert_eq!(rig.surface.pixels()[50], 128, "the byte as it stands, not added");
    }

    #[test]
    fn the_refill_is_one_band_down_reads_a_byte_past_fbuf_and_stops_at_390() {
        let mut part = synthetic();
        // A marker at fbuf[10] proves the left shift; scp's low byte is what
        // the `move` reads past the array.
        part.fbuf[10] = 0xAB;
        part.scp = 5;
        part.font[5] = 0x11;
        part.font[33 * MIEKKA_WIDTH + 5] = 0x22;
        part.shift_and_fill();
        assert_eq!(part.fbuf[9], 0xAB, "`:183` shifts one byte left");
        assert_eq!(part.fbuf[FBUF_STRIDE], 0x11, "x=0 lands at 158, not 0");
        assert_eq!(part.fbuf[FBUF_STRIDE + 33 * FBUF_STRIDE], 0x22, "x=33 at 5372");
        assert_eq!(part.fbuf[FBUF_LEN], 5, "the byte past the array is scp's low byte");
        assert_eq!(part.scp, 6);

        // Index 0 never receives a sample.
        part.fbuf = vec![0; FBUF_LEN + 1];
        part.font[6] = 0x44;
        part.shift_and_fill();
        assert_eq!(part.fbuf[0], 0, "no sample is written at index 0");
        assert_eq!(part.fbuf[FBUF_STRIDE], 0x44);

        // `:185`: `scp` stops at 390 with a 400-wide source.
        part.scp = SCP_MAX;
        part.font[SCP_MAX as usize] = 0x33;
        part.shift_and_fill();
        assert_eq!(part.scp, SCP_MAX);
        assert_eq!(part.fbuf[FBUF_STRIDE], 0x33, "the last column read is 390");
    }

    // --- the release's confirmation (A18) ------------------------------------

    #[test]
    fn the_releases_confirmation_starts_the_main_loop_seven_ticks_after_the_gate() {
        // The order changes at tick 647, the reference's published row-16
        // frame. The seventh confirming read is tick 653, and the first draw
        // is 654: boot 13616.
        let (mut rig, mut part) = started_with(through_stage_two(647, None, 1_600));
        run_to(&mut rig, &mut part, 653);
        assert_eq!(part.phase, Phase::Stage2);
        run_to(&mut rig, &mut part, 654);
        assert_eq!(part.phase, Phase::Main, "the seventh read does not draw");
        assert_eq!(part.sss, 0);
    }

    #[test]
    fn the_trees_immediate_gate_draws_the_keyframes_early() {
        // The counterfactual: with `confirm_ticks = 0` the main loop's first
        // draw is tick 648 (boot 13610), six draws before the release's.
        let tree = |tick: u32| -> Vec<u8> {
            let (mut rig, mut part) = started_with(through_stage_two(647, None, 2_000));
            part.confirm_ticks = 0;
            run_to(&mut rig, &mut part, tick + 1);
            assert_eq!(part.phase, Phase::Main);
            rig.rendered_rgb()
        };
        assert_eq!(
            differing_pixels(&tree(tick_of(14_000)), &reference("rayscrl", 14_000, "rgb")),
            8_375,
            "boot 14000 under the tree's immediate gate"
        );
        assert_eq!(
            differing_pixels(&tree(tick_of(14_500)), &reference("rayscrl", 14_500, "rgb")),
            11_958,
            "boot 14500 under the tree's immediate gate"
        );
    }

    // --- the keyframe residue ------------------------------------------------

    /// Replay the main loop's first `draws+1` draws over the background,
    /// returning the index plane and, per video offset, the `(band, source
    /// column)` of the font byte that last wrote it (`None` for the
    /// background).
    fn screen_provenance(part: &Rayscrl, draws: u32) -> (Vec<u8>, Vec<Option<(usize, usize)>>) {
        let mut fbuf = vec![0u8; FBUF_LEN + 1];
        let mut tags = vec![None; FBUF_LEN + 1];
        let mut screen = part.background.clone();
        let mut source = vec![None; VIDEO_BYTES];
        let mut scp = 0u16;
        for draw in 0..=draws {
            let sss = (draw % 3) as usize;
            for column in 0..FBUF_COLUMNS {
                let value = fbuf[column];
                for &offset in part.lists[sss].column(column) {
                    let at = offset as usize;
                    if value == 0 {
                        screen[at] = part.background[at];
                        source[at] = None;
                    } else {
                        screen[at] = value;
                        source[at] = tags[column];
                    }
                }
            }
            if sss == 2 {
                fbuf[FBUF_LEN] = (scp & 0xff) as u8;
                fbuf.copy_within(1..=FBUF_LEN, 0);
                tags.copy_within(1..=FBUF_LEN, 0);
                for x in 0..FONT_BANDS {
                    let at = FBUF_STRIDE + x * FBUF_STRIDE;
                    fbuf[at] = part.font[x * MIEKKA_WIDTH + scp as usize];
                    tags[at] = Some((x, scp as usize));
                }
                if scp < SCP_MAX {
                    scp += 1;
                }
            }
        }
        (screen, source)
    }

    /// `rayscrl.md` §6's five pairs: the pixels that differ from the reference
    /// at 14000 and 14500 draw from MIEKKA.SCI's strip, and the reference's
    /// byte is the tree's previous source column.
    #[test]
    fn the_keyframe_residue_is_the_releases_font_strip() {
        // The inverse of the 6→8-bit widening, to read a reference RGB triple
        // back to the palette index it names.
        let mut inverse = [None; 256];
        for value in 0..64u8 {
            inverse[dac_expand(value) as usize] = Some(value);
        }
        let index_of = |rig: &Rig<PlayerMusic>, rgb: &[u8]| -> Option<u8> {
            let six = [
                inverse[rgb[0] as usize]?,
                inverse[rgb[1] as usize]?,
                inverse[rgb[2] as usize]?,
            ];
            (0..=255u8).find(|index| rig.palette.entry(*index) == six)
        };

        let mut recovered = Vec::new();
        for (index, draws) in [(14_000u32, 384u32), (14_500, 884)] {
            let (mut rig, mut part) = started_real();
            run_to(&mut rig, &mut part, tick_of(index) + 1);
            let (plane, source) = screen_provenance(&part, draws);
            assert_eq!(plane, rig.surface.pixels()[..VIDEO_BYTES], "the replay at boot {index}");
            let theirs = reference("rayscrl", index, "rgb");
            let mine = rig.rendered_rgb();
            for (at, rgb) in theirs.chunks_exact(3).enumerate() {
                if mine[at * 3..at * 3 + 3] == *rgb {
                    continue;
                }
                let (band, scp) = source[at].expect("a font byte, not the background");
                let tree_byte = part.font[band * MIEKKA_WIDTH + scp];
                // The reference's byte: 0 (the background replay) where its
                // colour is the background's, a palette index otherwise.
                let background = rig.palette.entry(part.background[at]).map(dac_expand);
                let release = if background == *rgb {
                    0
                } else {
                    index_of(&rig, rgb).expect("the reference colour is a palette entry")
                };
                recovered.push((band, scp, tree_byte, release));
            }
        }
        recovered.sort_unstable();
        recovered.dedup();
        let expected: Vec<(usize, usize, u8, u8)> = vec![
            (13, 204, 245, 0), // the release's byte is 0: the background replay
            (14, 202, 219, 223),
            (14, 203, 223, 219),
            (15, 35, 210, 211),
            (15, 202, 223, 228),
        ];
        assert_eq!(recovered, expected);
        let font = read_asset(NAME, MIEKKA_PATH).unwrap();
        for &(band, source, _, release) in &recovered {
            if release != 0 {
                assert_eq!(
                    release,
                    font[778 + band * MIEKKA_WIDTH + source - 1],
                    "the release's byte is the tree's previous source column"
                );
            }
        }
    }
}
