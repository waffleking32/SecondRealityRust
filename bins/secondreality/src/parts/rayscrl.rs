//! RAYSCRL — the raytraced mirror-ball scroll.
//!
//! `WATER/DEMO.PAS` (190 lines) over `WATER/ROUTINES.ASM` (41 lines — the
//! colour-keyed column blitter) and the linked assets `BKG.CLX` (`BKR.PAS`'s
//! `_tausta`), `MIEKKA.SCI` (`MIEK.PAS`'s `_miekka`) and `WAT1-3.DAT`
//! (`T1-3.PAS`). Its acts, in order:
//!
//! 1. `:90-105` — mode 13h, `dis_partstart`, and a busy-wait for
//!    `dis_musplus() >= 0`, storing the order as `co`.
//! 2. `:107-116` — `fbuf` zeroed; `MIEKKA.SCI`'s palette and its first
//!    `400*34` pixel bytes loaded (the file carries 35 rows); the DAC blacked;
//!    `BKG.CLX`'s 64,000-byte picture rebased and blitted; the fade tables.
//! 3. `:117-132` — 127 frames of fade-in, the palette uploaded and stepped on
//!    odd frames, `scr(sss)` every frame with `sss` cycling 0, 1, 2.
//! 4. `:135-143` — a spin until the order changes from `co` **and** the row is
//!    at least 16, with no `waitb`.
//! 5. `:155-189` — the main loop: one `scr(sss)` a tick, the font shifted and
//!    refilled from `MIEKKA.SCI` every third tick, `scp` advancing to 390, the
//!    fade-out on `dis_musplus() == -11` exactly, and the exit on `dis_exit`
//!    or `quit`.
//!
//! # The blitter, unit for unit
//!
//! `Putrouts1` (`ROUTINES.ASM:12-40`) walks the `158*34` columns of one
//! position list. Each column is a count and then that many **byte offsets**
//! into video memory; a zero count is the original's hidden column (`:23-26`).
//! For each offset `di`:
//!
//! ```text
//! al = fs:[bx]                     ; this column's font byte
//! if al == 0 then al = gs:[di]     ; ... a zero replays the background pixel
//! es:[di] = al
//! ```
//!
//! — a colour key, not an add: the port does not carry MNTSCRL's +128 bias.
//! `scr` (`DEMO.PAS:51-80`) picks one of the three lists. Unlike MNTSCRL's, the
//! shift and refill are **not** in `scr`; they are in the main loop
//! (`:183-185`), after the `scr(2)` that drew with the old font.
//!
//! # The tick map, and the reference's numbers
//!
//! A tick is one `waitr` (`:13-20`, `dis_waitb`). Tick 0 is the first tick of
//! the init wait, the tick the reference's published order 88 row 0 first
//! stands at (boot frame **12962**, `reference/music/second3b/trace.csv`). The
//! init wait's condition is confirmed for [`GATE_CONFIRM_TICKS`] ticks and the
//! blit lands on the tick after the confirmation, so:
//!
//! ```text
//! ticks 0-6     the init wait's seven confirming polls, drawing nothing
//! tick 7        the loads, the black DAC and the 64,000-byte blit (boot 12969)
//! ticks 8-134   the 127 fade iterations `:117-132`
//! ticks 135-653 the stage-2 spin `:135-143`, drawing nothing
//! tick 654      the main loop's first `scr` (boot 13616)
//! tick 1988     the cue: `musplus == -11` confirmed, the fade's first upload
//!               (boot 14950)
//! ticks 1988-2052 the 65-step fade-out; the part retires at boot 15014
//! ```
//!
//! **These numbers are measurements, not an anchor.** `shell::boot_anchor(
//! "rayscrl")` stays `None` (the part's start is the window `(12900, 13000]`,
//! `reference-indices.md` §15), a run of this part numbers its own frames from
//! zero, and its manifest says `index_origin: "part"`, `anchor: null`. The
//! mapping the comparison uses is stated in the tests and in
//! `docs/fidelity/rayscrl.md`: **boot = 12962 + tick**, pinned by the fade-in's
//! recorded palette at 13000 (35 colours, mean 23.1535 — exactly the k=14
//! upload), the main-loop font phase at 14000/14500 (384 and 884 draws after
//! the first at 13616), and the fade-out's first upload at 14950.
//!
//! # The music, and the release's leave timings
//!
//! The part is mid-block in block 3, so `shell::music_for_part` seeks
//! `MUSIC1` to order **86** — the order the trace holds when the part's start
//! window opens — and `init` pre-polls the [`MUSIC_PRE_ROLL`] frames that
//! carry the player through order 86's end and the `254` marker at order-list
//! entry 87 into order 88 row 0. The seek to 86 is load-bearing: it is what
//! sets `zplus` bit 1 (the marker) that the reference's natural advance into
//! order 88 carries, and a seek straight to 88 leaves `zplus` 0, so the init's
//! `musplus >= 0` wait would never pass.
//!
//! **The tree's gates act too early.** Each of the tree's three waits fires on
//! the first tick its condition holds. The release's fires seven ticks later
//! on the port's clock (the cue's trace labels are one poll ahead of the port's
//! player): the blit is at boot 12969 (the published 88:0 stands from 12962),
//! the main loop starts at 13616 (the published order-89 row 16 stands from
//! 13609) and the fade's first upload is at 14950 (`musplus == -11` stands
//! from 14942 there; the port's player publishes it at 14943).
//! All three are reproduced by one measured rule, [`GATE_CONFIRM_TICKS`]: the
//! condition must be read true on seven consecutive ticks, and the gate acts
//! on the tick after. This is D24's single-constant branch — one rule, every
//! picture accounted for — and `confirm_ticks` keeps the tree's immediate
//! condition as the counterfactual the tests measure. See
//! `docs/fidelity/APPROXIMATIONS.md` A18.
//!
//! # The bugs, preserved
//!
//! - **The fade loops run `pf := 0 to 3`** (`:125`, `:174`), so `pal[x*3+3]` —
//!   the *next* entry's red — is incremented or decremented by the current
//!   entry's pass, doubling every red's ramp. The loop also touches
//!   `pal[768]`/`tmppal[768]` at x=255; in the original `pal[768]` is
//!   `font[0]` (the next global) and `tmppal[768]` is `pal[0]`. The port keeps
//!   the 769th byte of each array and the alias in [`ramp`]; the condition is
//!   false for the real palette (both bytes stay 0), so the font is untouched.
//! - **The insert index is `158 + x*158`** (`:184`), one band below the band
//!   each sample belongs to; index 0 never receives a sample. Kept in
//!   [`shift_and_fill`].
//! - **`move(fbuf[1], fbuf, sizeof(fbuf))`** (`:183`) copies 5,373 bytes, one
//!   past the declared `array[0..158*34]` — the next word is `scp`, so the
//!   byte read is `scp`'s low byte. The port keeps the extra byte and copies
//!   it; the refill loop then overwrites its destination. Preserved in
//!   [`shift_and_fill`].
//! - **`if scp < 390 then inc(scp)`** (`:185`) stops ten columns short of the
//!   400-wide source. Kept.
//!
//! # The assets
//!
//! All three are read in their original formats: `BKG.CLX` and `MIEKKA.SCI` are
//! RIX3 (`sr_assets::load_path`) and the three lists are the shared [`PosList`]
//! decoder. `MIEKKA.SCI` declares 400×250 and carries 35 whole rows; the
//! decoder delivers those 35 rows, and the part uses the first `400*34` bytes
//! the source's `move` reads (`:109`). Its palette wins over `BKG.CLX`'s: 64
//! entries differ, and the reference's 13500 frame uses MIEKKA's.
//!
//! A RIX3 file's palette is six bits; the part's fade is a byte ramp over it,
//! and the engine widens to eight only at upload.


use sr_assets::poslist::PosList;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{asset, load_image, mode_view, read_asset};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "rayscrl";

// ------------------------------------------------------------- the screen --

/// `DEMO.PAS:91` `mov ax,$13`: 320 pixels a row, 200 rows, one byte a pixel.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// Mode 13h's row stride, which is also the display's.
const LINE_LEN: u16 = 320;
/// The displayed picture: `:113`'s 64,000-byte blit.
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
/// What the blitter can address: `di` is a 16-bit word (`ROUTINES.ASM:27`).
const FRAME_MEMORY: usize = 65_536;

// ------------------------------------------------------------- the assets --

/// `DEMO.PAS:112-113` `_tausta`: 320x200, palette at +10, picture at +778.
const BKG_PATH: &str = "assets/WATER/BKG.CLX";
const BKG_WIDTH: usize = 320;
const BKG_HEIGHT: usize = 200;
/// `DEMO.PAS:108-109` `_miekka`: palette at +10, pixels at +778.
const MIEKKA_PATH: &str = "assets/WATER/MIEKKA.SCI";
const MIEKKA_WIDTH: usize = 400;
/// The rows the file carries (the decoder's `min(header, payload rows)`).
const MIEKKA_ROWS: usize = 35;
/// `:109` `move(...,font,400*34)`: the bytes the part loads.
const FONT_LOADED: usize = MIEKKA_WIDTH * 34;
/// `:38` `font : array[0..400*35] of byte`: the declared buffer, 14,001 bytes.
const FONT_LEN: usize = MIEKKA_WIDTH * MIEKKA_ROWS + 1;
/// The RIX3 layout the raw reads depend on: 10-byte header, 768-byte palette.
#[cfg(test)]
const RIX3_PALETTE: usize = 10;
/// After `:112`'s rebase the `_tausta` segment holds the picture's 64,000
/// bytes; the blitter's largest offset (63,999) is inside it. The port keeps
/// the trailing 778 bytes where the rebase leaves them, as MNTSCRL does.
const BACKGROUND_LEN: usize = VIDEO_BYTES + 778;

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
/// The source stride of the `MIEKKA.SCI` strip.
const FONT_STRIDE: usize = MIEKKA_WIDTH;
/// `:184` `for x := 0 to 33`: the bands the refill writes.
const FONT_BANDS: usize = 34;
/// `:184` `158 + x*158`: the refill's stride — one band down.
const FBUF_STRIDE: usize = 158;
/// `:185` `if scp < 390 then inc(scp)`.
const SCP_MAX: u16 = 390;

// ------------------------------------------------------------ the palette --

/// `:117` `for y := 0 to 63*2`: 127 fade-in iterations, not 128 — `63*2` is
/// 126 and the loop is inclusive.
const FADE_IN_STEPS: u32 = 127;
/// `:170` `if fp = 64 then quit := 1 else inc(fp)`: the fade-out ends on its
/// 65th tick.
const FADE_OUT_STEPS: u32 = 65;
/// `:207` `for x := 0 to 255`: the entries the fade loops walk.
const PALETTE_ENTRIES: usize = 256;
/// `:37` `tmppal,pal : array[0..768]`: 769 bytes each. The 769th is the
/// source's aliasing bug, kept ([`ramp`]).
const PALETTE_LEN: usize = 769;

// ---------------------------------------------------------------- the music --

/// `:162` `cmp dx,-11 / jnz @a`: the fade begins when `dis_musplus()` is
/// **exactly** -11.
const CUE: i32 = -11;
/// The tree's stage-2 gate: `:139-141` `cmp cx,CO / je @a / cmp bx,16 / jl @a`
/// — the order differs from `co` and the row is at least this.
const STAGE2_ROW: u8 = 16;
/// The measured D24 rule: the release's gates act on the tick after their
/// condition has been read true on this many consecutive ticks. The tree acts
/// on the first true tick (`confirm_ticks` 0 in the counterfactual). Measured
/// from the published 88:0 at boot 12962 and the blit at 12969, the row-16 at
/// 13609 and the main loop at 13616, the -11 at 14942 (the port's player
/// publishes it at 14943) and the fade's first upload at 14950. See the module
/// docs and `docs/fidelity/rayscrl.md` §4.
const GATE_CONFIRM_TICKS: u8 = 7;
/// `MUSIC1` at order 86 is where the trace holds the sequence when the part's
/// window opens; [`Rayscrl::init`] pre-polls to the published order 88 row 0
/// that stands at tick 0. Measured: the player publishes 88:0 on its **269th**
/// poll after a seek to order 86, so the engine's tick-0 poll is the 269th and
/// 268 polls precede it.
const MUSIC_PRE_ROLL: u32 = 268;
/// The capture frame the port's tick 0 stands at — the published order 88
/// row 0's first frame. **A mapping, not an anchor.** A `#[cfg(test)]`
/// constant: only the tests compare against the reference by it, and the
/// module docs are where a reader of the part meets the mapping.
#[cfg(test)]
const REFERENCE_FIRST_INDEX: u32 = 12_962;

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
    /// `:37` `pal`: the fade's value array, 769 bytes.
    pal: Vec<u8>,
    /// `:37` `tmppal`: the fade's target array, 769 bytes.
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
    /// The cue's confirmation: the fadeout is armed and acts next tick.
    cue_armed: bool,
    /// How many consecutive true ticks a gate requires. The release's
    /// [`GATE_CONFIRM_TICKS`]; 0 is the tree's immediate condition, kept for
    /// the counterfactual tests.
    confirm_ticks: u8,
    phase: Phase,
    /// The part's tick counter, incremented at the top of `frame`: the call
    /// that draws dump index `n` leaves this at `n + 1`.
    tick: u32,
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
            tick: 0,
        }
    }

    /// Whether `condition` has now held for `confirm_ticks` consecutive ticks;
    /// the caller acts on the tick after this returns true.
    fn confirm(&mut self, condition: bool) -> bool {
        if self.confirm_ticks == 0 {
            // The tree's immediate condition, kept for the counterfactual.
            return condition;
        }
        if condition {
            self.confirm = self.confirm.saturating_add(1);
        } else {
            self.confirm = 0;
        }
        self.confirm == self.confirm_ticks
    }
}

impl Rayscrl {
    /// `:111-113`: the DAC black and the 64,000-byte picture blitted. The
    /// mode set's clear is the surface's first 64 KB.
    fn setup(&mut self, ctx: &mut PartCtx) {
        let memory = ctx.surface.pixels_mut();
        memory[..VIDEO_BYTES].fill(0);
        memory[..VIDEO_BYTES].copy_from_slice(&self.background[..VIDEO_BYTES]);
        // `:111` `for x := 0 to 255 do setrgb(x,0,0,0)`.
        ctx.set_palette(Palette::new());
    }

    /// `:120-131`: fade-in iteration `y` — on odd `y`, upload `pal` and step
    /// every channel one toward `tmppal`; then one `scr`, `sss` cycling.
    fn fade_in_tick(&mut self, ctx: &mut PartCtx, y: u32) {
        if y % 2 == 1 {
            let dac = ramp(&mut self.pal, &self.tmppal, Direction::Up);
            ctx.set_palette(dac);
        }
        self.scr(ctx, self.sss as usize);
        self.sss = if self.sss == 2 { 0 } else { self.sss + 1 };
    }

    /// `:167-178`: one fade-out tick — `fp` up (or `quit` on the 65th), then
    /// upload `pal` and step every channel one toward zero.
    fn fade_out_tick(&mut self, ctx: &mut PartCtx) {
        if self.fp == (FADE_OUT_STEPS - 1) as u16 {
            self.quit = true;
        } else {
            self.fp += 1;
        }
        let dac = ramp(&mut self.pal, &self.tmppal, Direction::Down);
        ctx.set_palette(dac);
    }

    /// `:51-80` plus `:179-186`: one main-loop tick.
    fn main_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if !self.fadeout {
            if self.cue_armed {
                // The confirmation is complete: the fade acts on this tick.
                self.fadeout = true;
                self.cue_armed = false;
            } else {
                // `:162` `cmp dx,-11 / jnz @a`.
                let reading = ctx.musplus();
                if self.confirm(reading == CUE) {
                    self.cue_armed = true;
                }
            }
        }
        if self.fadeout {
            self.fade_out_tick(ctx);
        }

        // `:179-181`.
        self.scr(ctx, self.sss as usize);
        if self.sss == 2 {
            // `:180-185`: `sss := 0`, the shift and refill, and `scp`'s step.
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

    /// `:51-80` `scr(pos)` — one list's columns into video memory. A zero font
    /// byte replays the background pixel (`ROUTINES.ASM:28-32`).
    fn scr(&mut self, ctx: &mut PartCtx, which: usize) {
        let memory = ctx.surface.pixels_mut();
        let list = &self.lists[which];
        for column in 0..FBUF_COLUMNS {
            let value = self.fbuf[column];
            for &offset in list.column(column) {
                let at = offset as usize;
                memory[at] = if value == 0 {
                    self.background[at]
                } else {
                    value
                };
            }
        }
    }

    /// `:183-185`: `move(fbuf[1],fbuf,sizeof(fbuf))`, the refill one band down,
    /// and `scp` toward its bound.
    fn shift_and_fill(&mut self) {
        // The `move` reads one byte past the declared array; in the original
        // that byte is the low half of `scp`, the next variable. The port keeps
        // it explicitly (index `FBUF_LEN`), then copies 5,373 bytes.
        self.fbuf[FBUF_LEN] = (self.scp & 0xff) as u8;
        self.fbuf.copy_within(1..=FBUF_LEN, 0);
        // `:184` `for x := 0 to 33 do fbuf[158+x*158] := font[x*400+scp]`.
        for x in 0..FONT_BANDS {
            self.fbuf[FBUF_STRIDE + x * FBUF_STRIDE] =
                self.font[x * FONT_STRIDE + self.scp as usize];
        }
        // `:185`.
        if self.scp < SCP_MAX {
            self.scp += 1;
        }
    }
}

/// Parse and bound one POS list, `bytes` as read from `path`.
fn pos_list(path: &str, bytes: &[u8]) -> Result<PosList, PartError> {
    let list = PosList::parse(bytes).map_err(|error| PartError {
        part: NAME,
        message: format!("{path}: {error}"),
    })?;
    // `scr` indexes the surface and the background with every offset
    // (:441); past the 64,000-byte picture the background read panics.
    // The bound belongs here, where the file is still named, not in
    // the blitter's hot path.
    for (column, offsets) in list.columns.iter().enumerate() {
        for &offset in offsets {
            if offset as usize >= VIDEO_BYTES {
                return Err(PartError {
                    part: NAME,
                    message: format!(
                        "{path}: column {column} holds offset {offset}, outside the \
                         {VIDEO_BYTES}-byte picture"
                    ),
                });
            }
        }
    }
    if list.columns.len() != FBUF_COLUMNS {
        return Err(PartError {
            part: NAME,
            message: format!(
                "{path} holds {} columns; the blitter walks {FBUF_COLUMNS} (158*34, \
                 WATER/ROUTINES.ASM:21)",
                list.columns.len()
            ),
        });
    }
    Ok(list)
}

impl Part for Rayscrl {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `:98-116`: the music wait's pre-roll, the three assets and the display
    /// state. The source's wait and loads run before its first `waitr`, so the
    /// port's `init` reads the assets and `frame` runs the wait.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // Every gate this part has is music: the init wait (`:98-104`), stage 2
        // (`:135-143`) and the cue (`:158-166`). A run with no player could
        // never leave any of them.
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (WATER/DEMO.PAS:98-104, :135-143, :158-166) and the run has no music \
             to wait on",
        )?;

        // The reference's player was already at order 88 row 0 on the tick this
        // part's tick 0 stands at (`MUSIC_PRE_ROLL`); an unanchored entry run
        // starts the player with the part, so the difference is made here. A
        // part reached mid-chain finds the player already past this stretch,
        // so the catch-up is the entry run's (`music_fresh`) alone.
        if ctx.music_fresh() {
            for _ in 0..MUSIC_PRE_ROLL {
                if let Some(music) = ctx.music() {
                    music.poll();
                }
            }
        }

        // `:108` the palette and `:109` the first 400*34 pixels of MIEKKA.SCI.
        // Its declared 400x250 canvas carries 35 whole rows; the decoder hands
        // those back, and the part reads 34 of them.
        let miekka = load_image(NAME, MIEKKA_PATH)?;
        if (miekka.width() as usize, miekka.height() as usize) != (MIEKKA_WIDTH, MIEKKA_ROWS) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{MIEKKA_PATH} is {}x{}; the part reads {MIEKKA_WIDTH}x{MIEKKA_ROWS}",
                    miekka.width(),
                    miekka.height()
                ),
            });
        }
        if miekka.pixels().len() < FONT_LOADED {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{MIEKKA_PATH} carries {} pixel bytes; the part reads {FONT_LOADED} (400*34)",
                    miekka.pixels().len()
                ),
            });
        }
        self.pal = vec![0; PALETTE_LEN];
        for index in 0..PALETTE_ENTRIES {
            let rgb = miekka.palette.entry(index as u8);
            self.pal[index * 3..index * 3 + 3].copy_from_slice(&rgb);
        }
        // `:115-116` `move(pal,tmppal,768); fillchar(pal,768,#0)`. The 769th
        // byte of `pal` is `font[0]` in the original's segment and `tmppal`'s is
        // `pal[0]`; the port keeps the first at zero and reads the second as the
        // alias in `ramp`.
        self.tmppal = self.pal.clone();
        self.pal[..PALETTE_ENTRIES * 3].fill(0);
        self.font = vec![0; FONT_LEN];
        self.font[..FONT_LOADED].copy_from_slice(&miekka.pixels()[..FONT_LOADED]);

        // `:112-113` `_tausta`: the picture at +778 rebased to +0, then blitted.
        let bkg = load_image(NAME, BKG_PATH)?;
        if (bkg.width() as usize, bkg.height() as usize) != (BKG_WIDTH, BKG_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{BKG_PATH} is {}x{}; the part reads {BKG_WIDTH}x{BKG_HEIGHT}",
                    bkg.width(),
                    bkg.height()
                ),
            });
        }
        let mut background = Vec::with_capacity(BACKGROUND_LEN);
        background.extend_from_slice(&bkg.pixels()[..VIDEO_BYTES]);
        background.extend_from_slice(&bkg.pixels()[VIDEO_BYTES - 778..VIDEO_BYTES]);
        self.background = background;

        // `:51-80`: the three lists, one frame group each, 158*34 columns.
        self.lists = Vec::with_capacity(3);
        for rel in POS_PATHS {
            let bytes = read_asset(NAME, rel)?;
            self.lists.push(pos_list(&asset(rel), &bytes)?);
        }

        // `:39` `fbuf`, zeroed at `:107`, plus the byte the `move` reads past it.
        self.fbuf = vec![0; FBUF_LEN + 1];
        self.co = 0;
        self.scp = 0;
        self.sss = 0;
        self.fp = 0;
        self.fadeout = false;
        self.quit = false;
        self.confirm = 0;
        self.cue_armed = false;
        self.confirm_ticks = GATE_CONFIRM_TICKS;
        self.phase = Phase::Init;
        self.tick = 0;

        // The blitter addresses 64 KB of video memory flat, and `scr` also
        // reads the background at those offsets.
        if ctx.surface.pixels().len() < FRAME_MEMORY {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "the blitter addresses {FRAME_MEMORY} bytes of video memory and the surface \
                     holds {}",
                    ctx.surface.pixels().len()
                ),
            });
        }

        // `:90-92`: mode 13h and the display state a mode set leaves.
        *ctx.view = mode_view(SCREEN_WIDTH, SCREEN_HEIGHT, Some(LINE_LEN), Some(0));
        Ok(())
    }

    /// One tick of `DEMO.PAS:98-189`. See the module docs' tick map.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Init => {
                // `:98-104`: `musplus >= 0`, confirmed. The source never
                // checks `dis_exit` here; the port does not spin.
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                let reading = ctx.musplus();
                if self.confirm(reading >= 0) {
                    self.phase = Phase::Setup;
                }
                PartStatus::Continuing
            }
            Phase::Setup => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                // `:103` `mov co,cx`: the published order at the gate's leave.
                self.co = u16::from(ctx.music().map_or(0, |music| music.position().order));
                self.setup(ctx);
                self.phase = Phase::FadeIn { y: 0 };
                PartStatus::Continuing
            }
            Phase::FadeIn { y } => {
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
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
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                let (order, row) = ctx.music().map_or((0, 0), |music| {
                    let position = music.position();
                    (position.order, position.row)
                });
                let ready = u16::from(order) != self.co && row >= STAGE2_ROW;
                if self.confirm(ready) {
                    // `:149-154`: `sss`, `scp`, `fp`, `quit`, `fadeout` reset
                    // and `tmppal` zeroed, then the main loop's first `waitr`
                    // is the next tick.
                    self.sss = 0;
                    self.scp = 0;
                    self.fp = 0;
                    self.fadeout = false;
                    self.quit = false;
                    self.confirm = 0;
                    self.cue_armed = false;
                    self.tmppal[..PALETTE_ENTRIES * 3].fill(0);
                    self.phase = Phase::Main;
                }
                PartStatus::Continuing
            }
            Phase::Main => self.main_tick(ctx),
        }
    }
}

/// Which way a fade step moves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

/// One fade step: build the DAC from `pal` as the source uploads it, then step
/// each channel one toward the target — or toward zero for the fade-out.
///
/// The upload and the step are one loop (`:122-126`, `:171-175`), and that
/// order is load-bearing: entry `x` is uploaded before its own step but after
/// entry `x-1`'s `pf = 3` step, which is why the displayed red runs one ahead
/// of green and blue. The `pf` loop runs `0 to 3`, so `pal[x*3+3]` — the next
/// entry's red — is stepped too, and every red advances or retreats twice a
/// pass except entry 0's. At `x = 255` the loop touches index 768: in the
/// original `pal[768]` is `font[0]` and `tmppal[768]` is `pal[0]`; the port
/// keeps the byte and reads the alias. The real palette never satisfies the
/// comparison there (both are 0), so the font is untouched.
fn ramp(pal: &mut [u8], tmppal: &[u8], direction: Direction) -> Palette {
    let mut dac = Palette::new();
    for entry in 0..PALETTE_ENTRIES {
        dac.set_entry(
            entry as u8,
            [pal[entry * 3], pal[entry * 3 + 1], pal[entry * 3 + 2]],
        );
        for pf in 0..=3 {
            let at = entry * 3 + pf;
            let target = if at == PALETTE_LEN - 1 {
                pal[0]
            } else {
                tmppal[at]
            };
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
    use crate::parts::test_rig::{
        differing_bytes, differing_pixels, reference, Reading, Rig, RigMusic, ScriptedMusic,
    };
    use sr_engine::dac_expand;
    use sr_engine::music::Music;

    /// The tick the port's published `musplus` first stands at -11. The
    /// reference's trace holds it from boot 14942; the port's player publishes
    /// the same state one poll later (the 88:0 row's phase), at boot 14943.
    const CUE_FIRST: u32 = 1_981;
    /// The tick the fade's first upload lands on: boot 14950.
    const FADE_FIRST: u32 = 1_988;

    /// A player scripted one `(order, row, musplus)` reading a tick; a tick
    /// past the script reads 88:0 with `musplus` 0.
    fn scripted(script: Vec<(u8, u8, i32)>) -> ScriptedMusic {
        let reading = |(order, row, musplus)| Reading { order, row, musplus, sync: 0 };
        ScriptedMusic::readings(script.into_iter().map(reading).collect())
            .past_end(reading((88, 0, 0)))
            .never_syncs("`DEMO.PAS` never syncs: its music calls are `dis_musplus` and `dis_muscode`")
    }

    /// The init passes (`musplus` 0) and the stage-2 gate never sees the
    /// order change.
    fn quiet(last_tick: u32) -> ScriptedMusic {
        scripted(vec![(88, 0, 0); last_tick as usize + 1])
    }

    /// A script whose stage 2 passes at `stage_tick` (order 89, row 16)
    /// and whose cue is `-11` from `cue_tick`.
    fn through_stage_two(stage_tick: u32, cue_tick: Option<u32>, last_tick: u32) -> ScriptedMusic {
        let mut script = vec![(88, 0, 0); last_tick as usize + 1];
        for entry in script.iter_mut().skip(stage_tick as usize) {
            *entry = (89, 16, 0);
        }
        if let Some(cue) = cue_tick {
            for entry in script.iter_mut().skip(cue as usize) {
                *entry = (91, 53, -11);
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

    /// A part with the **real** player: `MUSIC1` seeked to order 86, the
    /// adapter the engine polls once before each tick. This is what a
    /// `--part rayscrl` run hands the part.
    fn started_real() -> (Rig<crate::audio::PlayerMusic>, Rayscrl) {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(86);
        let mut rig = rig(PlayerMusic::new(player, 0));
        let mut part = Rayscrl::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn run_ticks<M: RigMusic>(rig: &mut Rig<M>, part: &mut Rayscrl, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            rig.player().poll_for_tick();
            assert_eq!(
                part.frame(&mut rig.ctx(at)),
                PartStatus::Continuing,
                "tick {at}"
            );
        }
    }

    fn run_to<M: RigMusic>(rig: &mut Rig<M>, part: &mut Rayscrl, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    // --- the cue and the exit ------------------------------------------------

    #[test]
    fn the_gate_confirmation_is_the_measured_seven_ticks() {
        assert_eq!(GATE_CONFIRM_TICKS, 7);
        // The reference's three leaves, from the measured mapping:
        // the published 88:0 stands at 12962 and the blit at 12969 (7);
        // the row 16 at 13609 and the main loop at 13616 (7);
        // the -11 at 14942 and the fade's first upload at 14950 (8, of which
        // the player's own -11 is one poll late: 14943 + 7).
        assert_eq!(12_969 - REFERENCE_FIRST_INDEX, 7);
        assert_eq!(13_616 - 13_609, 7);
        assert_eq!(14_950 - 14_943, 7);
    }

    #[test]
    fn a_skipped_cue_keeps_the_part_running_without_fading() {
        // A script whose `musplus` walks from -12 to -10 — the two values
        // either side of the cue — and never stands at -11. The part must keep
        // scrolling, return `Continuing` every tick, and never fade.
        let mut script = vec![(88, 0, 0); 2_500];
        for entry in script.iter_mut().skip(700) {
            *entry = (89, 16, 0);
        }
        for entry in script.iter_mut().skip(1_000).take(100) {
            *entry = (91, 52, -12);
        }
        for entry in script.iter_mut().skip(1_100) {
            *entry = (91, 54, -10);
        }
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, 1_300);
        assert!(!part.fadeout, "a skipped cue must not fade");
        assert_eq!(part.phase, Phase::Main);
        // The scroller is drawing: the screen is not the untouched background.
        let drawn = rig.surface.pixels()[..VIDEO_BYTES].to_vec();
        assert_ne!(drawn, part.background[..VIDEO_BYTES]);
    }

    #[test]
    fn the_cue_is_exactly_minus_eleven_and_fades_sixty_five_ticks() {
        // -11 exactly, confirmed seven ticks, then the 65-step fade: 64
        // increments and the 65th tick's `quit`.
        let (mut rig, mut part) = started_with(through_stage_two(
            700,
            Some(CUE_FIRST),
            FADE_FIRST + 200,
        ));
        run_to(&mut rig, &mut part, FADE_FIRST);
        assert_eq!(part.phase, Phase::Main);
        assert!(!part.fadeout, "the fade acts on the tick after the confirm");

        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        assert!(part.fadeout, "the eighth consecutive -11 starts the fade");
        assert_eq!(part.fp, 1);
        // The first upload is the fade-in's stored `pal` (its 63s), which the
        // screen has not shown since the fade-in's last upload (62s).
        assert_eq!(rig.palette.entry(39)[2], 63);

        let fade_tick = part.tick;
        for step in 2..FADE_OUT_STEPS {
            assert_eq!(
                part.frame(&mut rig.ctx(fade_tick + step - 1)),
                PartStatus::Continuing,
                "fade tick {step}"
            );
        }
        assert_eq!(part.fp, 64);
        assert_eq!(
            part.frame(&mut rig.ctx(fade_tick + FADE_OUT_STEPS - 1)),
            PartStatus::Finished
        );
        assert!(part.quit);
        assert_eq!(part.phase, Phase::Done);
        assert!(
            (0..256).all(|index| rig.palette.entry(index as u8) == [0, 0, 0]),
            "the last fade tick uploads black"
        );
        assert_eq!(part.frame(&mut rig.ctx(9_999)), PartStatus::Finished);
    }

    #[test]
    fn minus_ten_and_minus_twelve_never_trigger_the_cue() {
        let mut script = vec![(88, 0, 0); 2_500];
        for entry in script.iter_mut().skip(700) {
            *entry = (89, 16, 0);
        }
        for entry in script.iter_mut().skip(CUE_FIRST as usize) {
            *entry = (91, 52, -12);
        }
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, FADE_FIRST + 100);
        assert!(!part.fadeout, "-12 is not the cue");

        let mut script = vec![(88, 0, 0); 2_500];
        for entry in script.iter_mut().skip(700) {
            *entry = (89, 16, 0);
        }
        for entry in script.iter_mut().skip(CUE_FIRST as usize) {
            *entry = (91, 54, -10);
        }
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, FADE_FIRST + 100);
        assert!(!part.fadeout, "-10 is not the cue");
    }

    #[test]
    fn exit_requested_during_stage_two_finishes_on_that_tick() {
        // Stage 2 is the phase's only true spin and the source never checks
        // `dis_exit` in it. The port polls once a tick and must honour the
        // notice on the tick it arrives.
        let (mut rig, mut part) = started_with(quiet(1_000));
        run_to(&mut rig, &mut part, 200);
        assert_eq!(part.phase, Phase::Stage2);
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished
        );
        assert_eq!(part.phase, Phase::Done);
        // The init wait, the fade and the main loop honour it too.
        let (mut rig, mut part) = started_with(quiet(10));
        assert_eq!(
            part.frame(&mut rig.ctx_exit(0)),
            PartStatus::Finished,
            "the init wait"
        );
        let (mut rig, mut part) = started_with(quiet(10));
        run_to(&mut rig, &mut part, 20);
        assert!(
            matches!(part.phase, Phase::FadeIn { .. }),
            "tick 20 is a fade tick"
        );
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the fade"
        );
        let (mut rig, mut part) = started_with(through_stage_two(700, None, 900));
        run_to(&mut rig, &mut part, 720);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished,
            "the main loop"
        );
    }

    // --- the reference -------------------------------------------------------

    fn sci_byte(path: &str, at: usize) -> u8 {
        read_asset(NAME, path).unwrap()[at]
    }

    // --- the assets ----------------------------------------------------------

    #[test]
    fn the_assets_are_read_the_way_the_source_reads_them() {
        let (_, part) = started();

        // `:108-109`: MIEKKA.SCI is a 400x35 crop (the decoder's `min` of the
        // declared 400x250 and the 14,000 bytes carried). Its palette is the
        // part's; its first 400*34 pixel bytes are the font strip.
        let miekka = read_asset(NAME, MIEKKA_PATH).unwrap();
        assert_eq!(miekka.len(), 14_778);
        assert_eq!(part.tmppal.len(), PALETTE_LEN);
        assert_eq!(
            &part.tmppal[..PALETTE_ENTRIES * 3],
            &miekka[RIX3_PALETTE..RIX3_PALETTE + PALETTE_ENTRIES * 3],
            "MIEKKA.SCI's palette at +10"
        );
        // MIEKKA's palette wins over BKG.CLX's: 64 of the 256 entries differ.
        let bkg_bytes = read_asset(NAME, BKG_PATH).unwrap();
        let differing_entries = (0..PALETTE_ENTRIES)
            .filter(|index| {
                miekka[RIX3_PALETTE + index * 3..RIX3_PALETTE + index * 3 + 3]
                    != bkg_bytes[RIX3_PALETTE + index * 3..RIX3_PALETTE + index * 3 + 3]
            })
            .count();
        assert_eq!(differing_entries, 64);
        assert!(
            part.pal[..PALETTE_ENTRIES * 3]
                .iter()
                .all(|byte| *byte == 0),
            "`:116` fills pal with zero"
        );
        assert_eq!(part.font.len(), FONT_LEN);
        assert_eq!(
            &part.font[..FONT_LOADED],
            &miekka[778..778 + FONT_LOADED],
            "`:109` takes the first 400*34 bytes at +778"
        );
        assert!(part.font[FONT_LOADED..].iter().all(|byte| *byte == 0));

        // `:112-113`: the picture at +778 rebased to +0; the trailing 778 bytes
        // stay where the rebase leaves them.
        let bkg = read_asset(NAME, BKG_PATH).unwrap();
        assert_eq!(bkg.len(), 64_778);
        assert_eq!(part.background.len(), BACKGROUND_LEN);
        assert_eq!(
            &part.background[..VIDEO_BYTES],
            &bkg[778..778 + VIDEO_BYTES]
        );
        assert_eq!(
            &part.background[VIDEO_BYTES..],
            &bkg[778 + VIDEO_BYTES - 778..778 + VIDEO_BYTES]
        );

        // `:51-80`: three lists of 158*34 columns, 20,858 positions between
        // them, and the largest destination inside the picture. The per-file
        // byte sizes and position counts are the survey's
        // (`reference-indices.md` §15's asset table is the note's source for
        // them; the parser is what measures them here).
        assert_eq!(part.lists.len(), 3);
        let mut positions = 0usize;
        let mut per_list = Vec::new();
        let mut max_offset = 0u16;
        for (index, list) in part.lists.iter().enumerate() {
            assert_eq!(list.columns.len(), FBUF_COLUMNS, "list {index}");
            let mut count = 0usize;
            for column in &list.columns {
                count += column.len();
                if let Some(offset) = column.iter().max() {
                    max_offset = max_offset.max(*offset);
                }
            }
            per_list.push(count);
            positions += count;
        }
        for (((path, bytes), expected), measured) in POS_PATHS
            .iter()
            .zip([23_096usize, 23_034, 27_818])
            .zip([6_176usize, 6_145, 8_537])
            .zip(per_list.iter())
        {
            assert_eq!(
                std::fs::metadata(asset(path)).unwrap().len() as usize,
                bytes,
                "{path}"
            );
            assert_eq!(*measured, expected, "{path}");
        }
        assert_eq!(per_list, [6_176, 6_145, 8_537], "WAT1/2/3's positions");
        assert_eq!(positions, 20_858, "WAT1-3's positions, the survey's count");
        assert_eq!(max_offset, 63_999);
        assert!((max_offset as usize) < VIDEO_BYTES);
    }

    #[test]
    fn an_offset_past_the_picture_is_refused_not_indexed() {
        // `scr` indexes the surface and the background with each column's
        // offsets (:441): the picture is `VIDEO_BYTES` = 64,000 bytes, so a
        // 65,000 offset would panic there rather than draw. The file carries
        // the blitter's full column count too, or the count check would
        // answer instead of the offset bound.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes()); // column 0: one offset
        bytes.extend_from_slice(&65_000u16.to_le_bytes());
        for _ in 1..FBUF_COLUMNS {
            bytes.extend_from_slice(&0u16.to_le_bytes()); // an empty column
        }
        let shown = "POS.DAT";
        let error = pos_list(shown, &bytes)
            .expect_err("65,000 is outside the 64,000-byte picture");
        assert!(
            error.message.contains(shown),
            "the error names the file: {}",
            error.message
        );
        assert!(
            error.message.contains("column 0"),
            "the error names the column: {}",
            error.message
        );
        assert!(
            error.message.contains("65000"),
            "the error names the offset: {}",
            error.message
        );
    }

    #[test]
    fn the_trees_miekka_object_carries_the_files_bytes() {
        // The note's §6 absolute: `MIEKKA.OBJ` carries `MIEKKA.SCI`'s bytes
        // exactly. The OMF reader walks the object's LEDATA records; their
        // concatenation is the file the tree links.
        let obj = std::fs::read("../../upstream/SecondReality/WATER/MIEKKA.OBJ").unwrap();
        let embedded = sr_assets::omf::extract_ledata(&obj).unwrap();
        let file = read_asset(NAME, MIEKKA_PATH).unwrap();
        assert_eq!(embedded.len(), file.len());
        assert_eq!(embedded, file);
    }

    #[test]
    fn the_music_pre_roll_puts_the_reference_state_at_tick_zero() {
        // `play_song(86)` walks the `254` at order-list entry 87 and enters
        // order 88 with `zplus` 2. The published order 88 row 0 is the 269th
        // poll; the part's tick 0 is that poll, so 268 pre-polls precede it.
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;

        assert_eq!(MUSIC_PRE_ROLL, 268);
        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(86);
        let mut music = PlayerMusic::new(player, 0);
        for _ in 0..MUSIC_PRE_ROLL {
            music.poll();
        }
        assert_eq!(
            music.position().order,
            86,
            "268 polls is still the old pattern's publication"
        );
        // The engine's tick-0 poll is the 269th.
        music.poll();
        assert_eq!(
            (music.position().order, music.position().row),
            (88, 0),
            "the reference's blit-time published words"
        );
        assert_eq!(music.musplus(), 0, "`musplus >= 0` passes at tick 0");
        // Row 1 arrives on the row's eighth poll, the reference's 12970.
        for _ in 0..8 {
            music.poll();
        }
        assert_eq!((music.position().order, music.position().row), (88, 1));
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        // A part the block's chain reaches is handed a player that has
        // already run through this stretch; 268 more polls would replay it.
        // The pre-roll is the entry run's (`music_fresh`) alone.
        let mut rig = rig(quiet(0));
        let mut part = Rayscrl::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player().polls, 0, "a mid-chain start polls nothing");
    }

    // --- the fade ------------------------------------------------------------

    #[test]
    fn the_fade_in_is_one_hundred_twenty_seven_iterations_and_pins_tick_zero() {
        assert_eq!(FADE_IN_STEPS, 127);
        let (mut rig, mut part) = started();
        // The blit lands on the tick after the init's confirmation.
        run_to(&mut rig, &mut part, 7);
        assert_eq!(part.phase, Phase::Setup);
        assert_eq!(part.frame(&mut rig.ctx(7)), PartStatus::Continuing);
        assert_eq!(part.phase, Phase::FadeIn { y: 0 });
        assert_eq!(part.tick, 8);
        // The last iteration is y=126 at tick 134; stage 2 starts at 135.
        run_to(&mut rig, &mut part, 134);
        assert_eq!(part.phase, Phase::FadeIn { y: 126 });
        run_to(&mut rig, &mut part, 135);
        assert_eq!(part.phase, Phase::Stage2);

        // Boot 13000 is tick 38: the fade's y=30, the *hold* of the k=14
        // upload made at y=29. The reference frame is exactly this palette.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 39);
        assert_eq!(part.tick, 39);
        let mine = rig.rendered_rgb();
        let theirs = reference("rayscrl", 13_000, "rgb");
        assert_eq!(differing_bytes(&mine, &theirs), 0, "boot 13000 (tick 38)");
        // The reference's own description (`reference-indices.md` §15.1): 35
        // colours and mean byte 23.15. The port's frame is the same bytes, so
        // the same two numbers, computed here so the note can cite them.
        let colours = |frame: &[u8]| {
            let mut seen = std::collections::BTreeSet::new();
            for pixel in frame.chunks_exact(3) {
                seen.insert([pixel[0], pixel[1], pixel[2]]);
            }
            seen.len()
        };
        assert_eq!(colours(&theirs), 35);
        let mean =
            theirs.iter().map(|byte| u64::from(*byte)).sum::<u64>() as f64 / theirs.len() as f64;
        assert!((mean - 23.1535).abs() < 0.0001, "mean {mean}");
        assert_eq!(colours(&mine), 35);
        // The stored array is one step past the last upload: gb 63 where the
        // screen shows 62 (`:125`'s loop uploads before it steps).
        assert_eq!(part.pal[39 * 3 + 2], 15);
        assert_eq!(rig.palette.entry(39)[2], 14);

        // The same shortfall at the fade's end: the reference's 13500 palette
        // (`reference/parts/rayscrl/manifest.json`) is MIEKKA's with exactly
        // seven blue channels at 62 instead of 63 — the entries whose target
        // is 63, one step short of the last increment.
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../reference/parts/rayscrl/manifest.json").unwrap(),
        )
        .unwrap();
        let held = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["index"] == 13_500)
            .unwrap();
        let palette = held["palette"].as_object().unwrap();
        let mut short = Vec::new();
        for (index, rgb) in palette {
            let index: usize = index.parse().unwrap();
            let reference: Vec<u64> = rgb
                .as_array()
                .unwrap()
                .iter()
                .map(|channel| channel.as_u64().unwrap())
                .collect();
            let miekka = [
                u64::from(sci_byte(MIEKKA_PATH, RIX3_PALETTE + index * 3)),
                u64::from(sci_byte(MIEKKA_PATH, RIX3_PALETTE + index * 3 + 1)),
                u64::from(sci_byte(MIEKKA_PATH, RIX3_PALETTE + index * 3 + 2)),
            ];
            assert_eq!(
                reference[0], miekka[0],
                "entry {index}'s red is the palette's"
            );
            assert_eq!(reference[1], miekka[1]);
            if reference[2] != miekka[2] {
                assert_eq!((miekka[2], reference[2]), (63, 62), "entry {index}");
                short.push(index);
            }
        }
        assert_eq!(short.len(), 7, "seven blues stop one step short");
    }

    #[test]
    fn the_fade_loops_step_the_next_entrys_red_and_read_the_byte_past_the_array() {
        // `:125` / `:174` `for pf := 0 to 3`: index x*3+3 is the *next*
        // entry's red, so every red but entry 0's is stepped twice a pass.
        let mut pal = vec![0u8; PALETTE_LEN];
        let mut tmppal = vec![0u8; PALETTE_LEN];
        for entry in 1..10 {
            tmppal[entry * 3] = 63;
            tmppal[entry * 3 + 1] = 63;
            tmppal[entry * 3 + 2] = 63;
        }
        let _ = ramp(&mut pal, &tmppal, Direction::Up);
        assert_eq!(pal[3], 2, "entry 1's red is stepped by 0 and by 1");
        assert_eq!(pal[4], 1);
        assert_eq!(pal[5], 1);
        assert_eq!(pal[6], 2, "and entry 2's by 1 and by 2");
        assert_eq!(pal[3], pal[6]);

        // `x = 255`, `pf = 3` touches index 768. In the original `pal[768]` is
        // `font[0]` and `tmppal[768]` is `pal[0]`; the port reads the alias.
        // Entry 0's target is 0 in the real palette, so nothing is stepped —
        // but a synthetic `pal[0]` makes the alias observable.
        let mut pal = vec![0u8; PALETTE_LEN];
        let mut tmppal = vec![0u8; PALETTE_LEN];
        pal[0] = 5;
        tmppal[0] = 5;
        let _ = ramp(&mut pal, &tmppal, Direction::Up);
        assert_eq!(pal[768], 1, "the byte past the array is stepped");
        // And the real palette leaves it alone: pal[0] stays 0.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 136);
        assert_eq!(part.pal[0], 0);
        assert_eq!(part.pal[768], 0, "font[0] is never touched");
        assert_eq!(part.font[0], 0);
        assert!(rig.palette.entry(0) == [0, 0, 0]);
    }

    // --- the blitter ---------------------------------------------------------

    /// A part with synthetic assets: three lists whose columns hold a handful
    /// of offsets, a flat background and a font of known bytes.
    fn synthetic() -> Rayscrl {
        let mut columns = vec![Vec::new(); FBUF_COLUMNS];
        columns[0] = vec![10, 20];
        columns[1] = Vec::new(); // a zero count writes nothing
        columns[2] = vec![40];
        columns[3] = vec![50]; // fbuf byte 0: the background replays
        let lists = (0..3)
            .map(|_| PosList {
                columns: columns.clone(),
            })
            .collect();
        let mut part = Rayscrl::new();
        part.background = vec![7; BACKGROUND_LEN];
        part.fbuf = vec![0; FBUF_LEN + 1];
        part.font = vec![0; FONT_LEN];
        part.lists = lists;
        part
    }

    #[test]
    fn the_blitter_replays_the_background_where_the_font_byte_is_zero() {
        // `ROUTINES.ASM:28-32`: a nonzero font byte is written as it stands (a
        // colour key); a zero replays the background — there is no +128 bias.
        let mut rig = rig(quiet(0));
        let mut part = synthetic();
        part.fbuf[0] = 9;
        part.fbuf[2] = 0;
        part.fbuf[3] = 0;
        part.scr(&mut rig.ctx(0), 0);
        assert_eq!(rig.surface.pixels()[10], 9);
        assert_eq!(rig.surface.pixels()[20], 9);
        assert_eq!(rig.surface.pixels()[30], 0);
        assert_eq!(rig.surface.pixels()[40], 7, "font 0 replays the background");
        assert_eq!(rig.surface.pixels()[50], 7);

        // MNTSCRL's additive +128 would have made the background 7 into 135;
        // the colour key leaves it 128 as the font byte stands (and 7 where
        // the byte is 0).
        part.fbuf[3] = 128;
        part.scr(&mut rig.ctx(0), 0);
        assert_eq!(rig.surface.pixels()[50], 128);
    }

    #[test]
    fn the_refill_is_one_band_down_reads_a_byte_past_fbuf_and_stops_at_390() {
        let mut part = synthetic();
        // A marker at fbuf[10] proves the left shift, and scp's low byte is
        // what the `move` reads past the array.
        part.fbuf[10] = 0xAB;
        part.scp = 5;
        part.font[5] = 0x11;
        part.font[33 * FONT_STRIDE + 5] = 0x22;
        part.shift_and_fill();
        assert_eq!(part.fbuf[9], 0xAB, "`:183` shifts one byte left");
        assert_eq!(part.fbuf[10], 0, "and the seam is refilled where x=0");
        assert_eq!(part.fbuf[FBUF_STRIDE], 0x11, "x=0 lands at 158, not 0");
        assert_eq!(
            part.fbuf[FBUF_STRIDE + 33 * FBUF_STRIDE],
            0x22,
            "x=33 at 5372"
        );
        assert_eq!(
            part.fbuf[FBUF_LEN], 5,
            "the byte past the array is scp's low byte"
        );
        assert_eq!(part.scp, 6);

        // Index 0 never receives a sample: the refill's lowest write is 158.
        part.fbuf = vec![0; FBUF_LEN + 1];
        part.scp = 6;
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

    // --- the stage gate, and the release's confirmation ----------------------

    #[test]
    fn the_releases_confirmation_starts_the_main_loop_seven_ticks_after_the_gate() {
        // The script's order changes at tick 647, the reference's published
        // row-16 frame. The tree acts at once (the drawn frame is 648); the
        // release acts on the tick after seven confirming reads: the stage-2
        // tick that sees the seventh is 653, and the first draw is 654 —
        // boot 13616.
        let (mut rig, mut part) = started_with(through_stage_two(647, None, 1_600));
        run_to(&mut rig, &mut part, 653);
        assert_eq!(part.phase, Phase::Stage2);
        run_to(&mut rig, &mut part, 654);
        assert_eq!(part.phase, Phase::Main, "the seventh read does not draw");
        assert_eq!(part.tick, 654);
        assert_eq!(REFERENCE_FIRST_INDEX + 654, 13_616);
    }

    #[test]
    fn the_trees_immediate_gate_draws_the_keyframes_early() {
        // The counterfactual: `confirm_ticks = 0` is the tree's first-true
        // condition. The tree's first draw is boot 13610 (the stage-2 onset
        // is tick 647, the transition 647, the draw 648); the release's is
        // 13616, so the tree is **six draws early**. By boot 14000 the
        // reference has 385 draws (indices 0-384) and the tree's policy has
        // 391 (indices 0-390): the frame at tick 1038 is index 390, not 384.
        let tree = |tick: u32| -> Vec<u8> {
            let (mut rig, mut part) = started_with(through_stage_two(647, None, 2_000));
            part.confirm_ticks = 0;
            run_to(&mut rig, &mut part, tick + 1);
            assert_eq!(part.phase, Phase::Main);
            rig.rendered_rgb()
        };
        // The tree's first draw is boot 13610; the reference's is 13616. At
        // 14000 the strip has scrolled six draws further.
        assert_eq!(
            differing_pixels(&tree(1_038), &reference("rayscrl", 14_000, "rgb")),
            8_375,
            "boot 14000 under the tree's immediate gate"
        );
        assert_eq!(
            differing_pixels(&tree(1_538), &reference("rayscrl", 14_500, "rgb")),
            11_958,
            "boot 14500 under the tree's immediate gate"
        );
    }

    // --- the four keyframes --------------------------------------------------

    /// The mapping: boot = [`REFERENCE_FIRST_INDEX`] + tick. It is pinned by
    /// the fade-in's palette (boot 13000 = tick 38), the main-loop font phase
    /// (14000 = 1038, 14500 = 1538) and the fade-out's first upload
    /// (14950 = 1988). A mapping, not an anchor.
    fn tick_of(index: u32) -> u32 {
        index - REFERENCE_FIRST_INDEX
    }

    #[test]
    fn the_mapping_is_the_one_the_reference_pins() {
        assert_eq!(tick_of(13_000), 38);
        assert_eq!(tick_of(13_500), 538);
        assert_eq!(tick_of(14_000), 1_038);
        assert_eq!(tick_of(14_500), 1_538);
        assert_eq!(tick_of(14_950), 1_988);
    }

    /// Replay the main loop's first `draws+1` draws over the background,
    /// returning the index plane and, per video offset, the `(band, source
    /// column)` of the font byte that last wrote it (`None` for the
    /// background).
    fn screen_provenance(part: &Rayscrl, draws: u32) -> (Vec<u8>, Vec<Option<(usize, usize)>>) {
        let mut fbuf = vec![0u8; FBUF_LEN + 1];
        let mut tags = vec![None; FBUF_LEN + 1];
        let mut screen = part.background[..VIDEO_BYTES].to_vec();
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
                    fbuf[at] = part.font[x * FONT_STRIDE + scp as usize];
                    tags[at] = Some((x, scp as usize));
                }
                if scp < SCP_MAX {
                    scp += 1;
                }
            }
        }
        (screen, source)
    }

    #[test]
    fn the_keyframe_residue_is_the_releases_font_strip() {
        // The only pixels that differ from the reference at 14000 and 14500
        // draw from MIEKKA.SCI's strip, and at five source positions the
        // reference's byte is the tree's *previous* source column. This test
        // recovers those five pairs from the two frames, so the note's numbers
        // are produced, not asserted by hand.
        //
        // The inverse of the engine's 6→8-bit widening, for reading a
        // reference RGB triple back to the palette index it names.
        let mut inverse = [None; 256];
        for value in 0..64u8 {
            inverse[dac_expand(value) as usize] = Some(value);
        }
        let index_of = |rig: &Rig<crate::audio::PlayerMusic>, rgb: &[u8]| -> Option<u8> {
            let (r, g, b) = (
                inverse[rgb[0] as usize]?,
                inverse[rgb[1] as usize]?,
                inverse[rgb[2] as usize]?,
            );
            (0..=255u8).find(|index| {
                let entry = rig.palette.entry(*index);
                (entry[0], entry[1], entry[2]) == (r, g, b)
            })
        };

        let mut recovered = Vec::new();
        for (tick, index, draws) in [
            (tick_of(14_000), 14_000u32, 384u32),
            (tick_of(14_500), 14_500, 884),
        ] {
            let (mut rig, mut part) = started_real();
            run_to(&mut rig, &mut part, tick + 1);
            let (plane, source) = screen_provenance(&part, draws);
            assert_eq!(
                plane,
                rig.surface.pixels()[..VIDEO_BYTES],
                "the replay is the drawn screen at boot {index}"
            );
            let theirs = reference("rayscrl", index, "rgb");
            let mine = rig.rendered_rgb();
            for (at, rgb) in theirs.chunks_exact(3).enumerate() {
                if mine[at * 3..at * 3 + 3] == *rgb {
                    continue;
                }
                let (band, scp) = source[at].expect("a font byte, not the background");
                let tree_byte = part.font[band * FONT_STRIDE + scp];
                // The reference's byte: the background replay (0) where its
                // colour is the background's, a palette index otherwise.
                let background = rig.palette.entry(part.background[at]);
                let background_rgb = [
                    dac_expand(background[0]),
                    dac_expand(background[1]),
                    dac_expand(background[2]),
                ];
                let release = if background_rgb == *rgb {
                    0
                } else {
                    match index_of(&rig, rgb) {
                        Some(index) => index,
                        None => {
                            panic!("the reference colour at boot {index} ({at}) is a palette entry")
                        }
                    }
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
        for &(band, source, _, release) in &recovered {
            if release != 0 {
                let font = read_asset(NAME, MIEKKA_PATH).unwrap();
                assert_eq!(
                    release,
                    font[778 + band * FONT_STRIDE + source - 1],
                    "the release's byte is the tree's previous source column"
                );
            }
        }
    }

    #[test]
    fn the_four_keyframes_match_the_reference_at_the_stated_mapping() {
        // 13000 is the fade's hold (both planes compared only as RGB; the
        // reference files no `.idx` for it); 13500 is the held background and
        // the reference's one indexed keyframe; 14000 and 14500 are RGB-only.
        // The four are compared byte for byte, with D19's support: the frame
        // either side of each differs from the keyframe, so the mapping is
        // exact and not a search's loosest fit.
        let frame = |index: u32| -> (Rig<crate::audio::PlayerMusic>, Rayscrl) {
            let (mut rig, mut part) = started_real();
            run_to(&mut rig, &mut part, tick_of(index) + 1);
            (rig, part)
        };

        // 13000.
        let (rig, _) = frame(13_000);
        assert_eq!(
            differing_bytes(&rig.rendered_rgb(), &reference("rayscrl", 13_000, "rgb")),
            0,
            "boot 13000 (tick {})",
            tick_of(13_000)
        );

        // 13500, in both planes.
        let (rig, _) = frame(13_500);
        {
            let theirs = reference("rayscrl", 13_500, "rgb");
            let mut seen = std::collections::BTreeSet::new();
            for pixel in theirs.chunks_exact(3) {
                seen.insert([pixel[0], pixel[1], pixel[2]]);
            }
            assert_eq!(seen.len(), 190, "the survey's 190 colours");
            let mean = theirs.iter().map(|byte| u64::from(*byte)).sum::<u64>() as f64
                / theirs.len() as f64;
            assert!((mean - 27.8114).abs() < 0.0001, "mean {mean}");
        }
        assert_eq!(
            differing_bytes(&rig.rendered_indices(), &reference("rayscrl", 13_500, "idx")),
            0,
            "boot 13500's index plane"
        );
        assert_eq!(
            differing_bytes(&rig.rendered_rgb(), &reference("rayscrl", 13_500, "rgb")),
            0,
            "boot 13500's RGB"
        );

        // 14000 and 14500: the release's confirmation puts the main loop's
        // first draw at boot 13616, which is the phase the reference's font
        // scroll has. The residual is the release's embedded MIEKKA differing
        // from the tree's asset at the handful of bytes those frames expose
        // (see `docs/fidelity/rayscrl.md` §6).
        for (index, expected) in [(14_000u32, 3usize), (14_500, 42)] {
            let (rig, _) = frame(index);
            let key = rig.rendered_rgb();
            let theirs = reference("rayscrl", index, "rgb");
            assert_eq!(
                differing_pixels(&key, &theirs),
                expected,
                "boot {index} (tick {})",
                tick_of(index)
            );
            let positions: Vec<(u32, u32)> = key
                .chunks_exact(3)
                .zip(theirs.chunks_exact(3))
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(at, _)| ((at % 320) as u32, (at / 320) as u32))
                .collect();
            if index == 14_000 {
                assert_eq!(positions, [(2, 0), (3, 0), (4, 0)]);
            } else {
                // All 42 lie in the top-left corner the strip's first rows
                // have scrolled into: rows 0-5, columns 3-27.
                assert_eq!(positions.len(), 42);
                assert_eq!(positions.iter().map(|(x, _)| *x).min(), Some(3));
                assert_eq!(positions.iter().map(|(x, _)| *x).max(), Some(27));
                assert_eq!(positions.iter().map(|(_, y)| *y).min(), Some(0));
                assert_eq!(positions.iter().map(|(_, y)| *y).max(), Some(5));
            }
            // D19: the neighbours are different pictures.
            for neighbour in [tick_of(index) - 1, tick_of(index) + 1] {
                let (mut rig, mut part) = started_real();
                run_to(&mut rig, &mut part, neighbour + 1);
                assert_ne!(
                    rig.rendered_rgb(),
                    key,
                    "boot {index}: tick {neighbour} is not distinguishable"
                );
            }
        }
    }
}
