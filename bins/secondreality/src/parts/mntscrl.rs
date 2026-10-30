//! MNTSCRL — the mountain scroller: a landscape with a scrolling leaf overlay.
//!
//! `FOREST/READ2.PAS` (224 lines) is the part, over `FOREST/ROUTINES.ASM` (39
//! lines — the additive column blitter) and the linked assets `BGR.PAS`'s
//! `hback` (`FOREST/HILLBACK.CLX`, built by `FOREST/MAKEOB.BAT:1`), `AOS1-3.PAS`'s
//! `posi1-3` (`FOREST/POS1-3.DAT`), and `O2.SCI`, which the part reads at run
//! time (`:110`). Its acts, in order:
//!
//! 1. `:101-108` — mode 13h and `dis_partstart`.
//! 2. `:110-117` — 31 rows of 640 bytes read out of `O2.SCI` at `x*640+778`,
//!    into `fbuf`.
//! 3. `:119` — every non-zero byte of `fbuf` gets **+128** (`inc` wraps, and
//!    the loop runs to `sizeof(fbuf)`, one byte past the array — see
//!    [`apply_bias`]).
//! 4. `:120-123` — the palette from `hback+10`, the 64,000-byte picture from
//!    `hback+778` rebased to `hback+0`, the DAC all black, and the picture
//!    blitted to `$a000`.
//! 5. `:125-142` — 64 frames fading the DAC from black to `pal`, with entries
//!    0-31 and 128-159 held black.
//! 6. `:144-151` — `tmppal = pal`, `fpal = pal` with those bands black, an
//!    upload, and the font built: source columns 0-132 into font columns
//!    104-236 of each of the 31 rows, with `scp := 133`.
//! 7. `:153-159` — the first wait. The release's own frames measure it at
//!    **71 ticks**; the tree's `dis_musplus() >= 0` is kept as the measured
//!    counterfactual ([`Mntscrl::tree_wait`], [`RELEASE_WAIT_TICKS`]).
//! 8. `:165-182` — the 127-frame pre-roll: `scr(sss)` with `sss` cycling 0,1,2,
//!    the band ramp re-uploaded on odd frames.
//! 9. `:184-224` — the main loop: one `scr(sss)` a tick, the fade-out when
//!    `dis_musplus() == -11` exactly, and the exit on `dis_exit` or `quit`.
//!
//! # The blitter, unit for unit
//!
//! `Putrouts` (`ROUTINES.ASM:11-39`) walks the `237*31` columns of one
//! position list. Each column is a count and then that many **byte offsets**
//! into video memory; a zero count is the original's hidden column and is
//! skipped (`:23-26`). For each offset `di` it writes
//!
//! ```text
//! es:[di] = gs:[di] + fs:[bx]      ; the background byte + this column's font byte
//! ```
//!
//! with `bx` the column index (one font byte a column) and the add a byte add,
//! so it wraps at 256. A font byte of 0 is transparent in the additive sense:
//! the background shows through unchanged. `scr` (`READ2.PAS:56-91`) picks one
//! of the three lists; `scr(2)` additionally shifts the packed font one byte
//! left (`move(font[1],font[0],237*31)`, `:86`), appends the next `O2.SCI`
//! column at column 236 of every row (`:87`), and advances `scp` toward its
//! bound (`:88`; the release's 640, see [`SCP_MAX`]). The port keeps the font
//! as the same packed 31×237 array so the shift moves bytes across row
//! boundaries exactly as the source's `move` does before the append
//! overwrites the seam.
//!
//! # The tick map, and the reference's numbers
//!
//! A tick is one `waitr` (`:17-24`, `dis_waitb`); tick 0 is the code before
//! the first one, which is CRED's and ENDSCRL's convention too.
//!
//! ```text
//! tick 0        the mode set, the loads, the black DAC and the background blit
//! ticks 1-64    the 64 fade-in uploads (`:132-142`)
//! tick 64       also `:144-151`: the palette swap and the font build
//! ticks 65-135  the release's 71-tick wait — nothing drawn
//! ticks 136-262 the pre-roll (`:165-182`)
//! then          the main loop (`:193-224`), one tick a `scr`
//! ```
//!
//! The reference's own fade pins where tick 0 is: `docs/fidelity/reference-indices.md`
//! §11.1 records the frame means 6.08 at 425, 8.91 at 430, 10.25 at 435 and
//! 10.53 at 440, and the port's palette curve (`min(step, pal)` per channel)
//! puts those four on the steps 5, 10, 15 and 20 — one `n` for each, and
//! `frame = tick 0 + n + 1`, so **tick 0 is boot frame 419**. It is a
//! measurement, not an anchor: `shell::boot_anchor("mntscrl")` stays `None`
//! and a run of this part numbers its own frames from zero. The mapping is
//! what `docs/fidelity/mntscrl.md` uses to compare against the reference.
//!
//! # The music, and the release's timeline
//!
//! The block-3 restart (`MAIN/U2.ASM:856-863`) starts MUSIC1 at order 42 on
//! the music clock's first tick, frame 408 (`shell::MUSIC_START_FRAME`), so
//! the reference's part — whose tick 0 is boot 419 — already saw **11 polls**
//! of it. An unanchored run numbers the music with the part
//! (`MusicStart::new(None)`), so [`Mntscrl::init`] pre-polls those 11 frames:
//! the port's player is then where the reference's was, tick for tick. That
//! is the same `pre` a `MusicStart` anchored at 419 would compute; if this
//! part ever gains an anchor, the shell's pre-poll and this one would have to
//! become one.
//!
//! **The first wait follows the release, not the tree.** `READ2.PAS:153-159`
//! waits for `dis_musplus() >= 0`, and `reference/music/second3` never reads
//! that until frame 2488 (order 47) — over 1,900 frames after the release
//! leaves the wait. The release's own keyframes put its last held frame at
//! boot **554** and its pre-roll's first `scr` at 555, so with tick 0 at 419
//! the wait is **71 ticks** (65 through 135). It cannot be a `musplus`
//! threshold: the trace's position is order 42 row 16 on both sides of the
//! boundary (`zplus` 0, `musplus` -32 at both), so every threshold either
//! exits at tick 65 or never at 135. The port therefore runs the measured
//! duration ([`RELEASE_WAIT_TICKS`]), as the controller's D24 ruling requires
//! — the real run must match the reference's four keyframes — and
//! [`Mntscrl::tree_wait`] keeps the tree's condition so
//! `the_trees_own_wait_would_hold_through_the_scroll_keyframes` can measure
//! what the development snapshot would have drawn. The cue that *ends* the
//! part, `== -11` exactly (`:196-203`), is the tree's and is tested here.
//!
//! # The bugs, preserved
//!
//! - **`for x := 0 to sizeof(fbuf)`** (`:119`) runs one byte past the declared
//!   array `array[0..640*31]`. In the original that byte is `font[0]` (the next
//!   global), still zero at that point; the port models it as an explicit
//!   zero byte and documents the aliasing in [`apply_bias`].
//! - **The `veke = 2800` bound is dead** (`:3`, `:224`): `inc(frame)` is
//!   commented out (`:221`), so the `frame = veke` arm of the `until` can
//!   never fire. The port has no `frame` counter and runs until the cue, as
//!   the original does.
//! - **The palette bands 0-31 and 128-159 are forced black** during the
//!   fade-in (`:126-127`) and again after it (`:146-147`), and the pre-roll
//!   ramps them back to the picture's own values (`:165-178`) — which is the
//!   reference's "background appearing at 560-615". Both the forcing and the
//!   ramp are the source's, and both are kept.
//!
//! # The assets
//!
//! The three are read in their original formats: `O2.SCI` and `HILLBACK.CLX`
//! are RIX3 (`sr_assets::load_path`), whose palette and pixel payloads are the
//! bytes the original's raw `seek`s reach — the RIX3 reader's own tests pin
//! that — and `POS1-3.DAT` is the shared [`PosList`] decoder. The source's
//! rebase of `hback` (`:121`) is not performed: the port keeps the 64,000
//! picture bytes as the background and the asset's trailing 778 bytes where
//! the rebased memory leaves them, so an offset in the list's 64 KB range
//! reads what the original's segment held ([`BACKGROUND_LEN`]).


use sr_assets::poslist::PosList;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::{asset, load_image, read_asset};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "mntscrl";

// ------------------------------------------------------------- the screen --

/// `READ2.PAS:102` `mov ax,$13`: 320 pixels a row, 200 rows, one byte a pixel.
const SCREEN_WIDTH: u16 = 320;
const SCREEN_HEIGHT: u16 = 200;
/// Mode 13h's row stride, which is also the display's: `ROUTINES.ASM`'s `di`
/// offsets are `x + y*320`. The view names it explicitly so the surface's own
/// row stride (640) cannot shear the picture.
const LINE_LEN: u16 = 320;
/// The displayed picture: `:123`'s 64,000-byte blit.
const VIDEO_BYTES: usize = SCREEN_WIDTH as usize * SCREEN_HEIGHT as usize;
/// What the blitter can address: `di` is a 16-bit word (`ROUTINES.ASM:27`), so
/// video memory is the whole 64 KB and offsets past 64,000 write off screen.
const FRAME_MEMORY: usize = 65_536;

// ------------------------------------------------------------- the assets --

/// `:110` `assign(f,'o2.sci')`: 31 rows of 640 bytes at `x*640+778`.
const O2_PATH: &str = "assets/FOREST/O2.SCI";
const O2_WIDTH: usize = 640;
const O2_HEIGHT: usize = 32;
/// `:112-116`: the rows the part reads — 0 through 30, of the file's 32.
const FBUF_ROWS: usize = 31;
/// `:42` `fbuf : array[0..640*31] of byte` — 19,841 elements, of which the
/// load fills the first 19,840. The declared array is kept whole because
/// `:119`'s loop walks it.
const FBUF_LEN: usize = O2_WIDTH * FBUF_ROWS + 1;
/// The bytes the load actually fills.
const FBUF_LOADED: usize = O2_WIDTH * FBUF_ROWS;

/// `BGR.PAS:2-6` links `hillback.obj`; `:120-121` take its palette from +10 and
/// its 64,000-byte picture from +778.
const HILLBACK_PATH: &str = "assets/FOREST/HILLBACK.CLX";
const HILLBACK_WIDTH: usize = 320;
const HILLBACK_HEIGHT: usize = 200;
/// The RIX3 layout the raw reads depend on: 10-byte header, 768-byte palette,
/// pixels at 778 (`crates/sr-assets/src/rix.rs`).
#[cfg(test)]
const RIX3_PALETTE: usize = 10;
/// After `:121`'s rebase the `hback` segment holds the picture's 64,000 bytes
/// followed by the asset's own last 778 bytes (the rebase copies from +778 to
/// +0 and leaves the tail standing). The blitter reads `gs:[di]` from this
/// memory, so the port keeps all 64,778 bytes; the lists' largest offset is
/// 63,870 (`the_assets_are_read_the_way_the_source_reads_them`), inside the
/// picture.
const BACKGROUND_LEN: usize = VIDEO_BYTES + 778;

/// `AOS1-3.PAS` link the three lists; `READ2.PAS:59-90` picks one.
const POS_PATHS: [&str; 3] = [
    "assets/FOREST/POS1.DAT",
    "assets/FOREST/POS2.DAT",
    "assets/FOREST/POS3.DAT",
];
/// `ROUTINES.ASM:21` `mov dx,237*31`: the columns the blitter walks.
const FONT_COLUMNS: usize = 237;
const FONT_ROWS: usize = 31;
const FONT_LEN: usize = FONT_COLUMNS * FONT_ROWS;

// --------------------------------------------------------------- the font --

/// `:150` `move(fbuf[x*640], font[x*237+104], 133)`: the source columns
/// 0-132 land at font columns 104-236.
const FONT_OFFSET: usize = 104;
const FONT_FIRST_BYTES: usize = 133;
/// `:151` `scp := 133`.
const SCP_START: u16 = 133;
/// `:88`'s bound on `scp` — **640 in the release, where the tree says 639**.
///
/// The tree's line is `if scp < 639 then inc(scp)`, so its last appended
/// column is 639. The release reads one column further: at boot 2300 its
/// frame is byte-identical with this bound at 640 and differs in 401 bytes
/// under the tree's 639, while 1100 and 1700 — before the bound is reached —
/// are identical under both
/// (`the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`). The
/// tree's own `fbuf` is `array[0..640*31]`: index 19,840, one past the 31
/// loaded rows, is exactly the element a stride-640 read at column 640 needs
/// for row 30, and `:119`'s loop covers it. The extra element exists for this
/// read, so the release's bound is the array's own; `D24`'s single constant is
/// adopted, and `docs/fidelity/mntscrl.md` §7 records it.
const SCP_MAX: u16 = 640;
/// `:119` `inc(fbuf[x],128)`.
const BIAS: u8 = 128;

// ------------------------------------------------------------ the palette --

/// `:126-127` and `:146-147`: the two 32-entry bands forced black.
const BLACK_BANDS: [(u8, u8); 2] = [(0, 32), (128, 160)];
/// `:132` `for y := 0 to 63`: the fade-in's uploads.
const FADE_IN_STEPS: u32 = 64;
/// `:165` `for y := 0 to 63*2`: the pre-roll's iterations.
const PRE_ROLL_STEPS: u32 = 127;
/// `:171-177`: the pre-roll uploads and ramps entries 0 through 176.
const PRE_ROLL_ENTRIES: usize = 177;
/// `:207` `if fp = 64 then quit := 1 else inc(fp)`: the fade-out ends on its
/// 65th tick.
const FADE_OUT_STEPS: u32 = 65;

// ---------------------------------------------------------------- the cue --

/// `:200` `cmp dx,-11 / jnz @a`: the fade begins when `dis_musplus()` is
/// **exactly** -11 (`zplus` 1, row 53; `sr_engine::music::musplus_from`).
const CUE: i32 = -11;

/// The release's first wait, in ticks — the tree's `musplus >= 0` replaced by
/// the duration the release's own frames measure.
///
/// The reference's last held frame is boot **554** and its pre-roll's first
/// `scr` is boot **555** (`docs/fidelity/mntscrl.md` §6): with tick 0 at 419
/// the wait occupies ticks 65 through 135, so it lasts **71** ticks. No
/// `musplus` threshold can produce that boundary — at ticks 134 and 135 the
/// published position is order 42 row 16 on both sides (`zplus` 0), so
/// `musplus` is -32 at both, and every threshold either ends the wait at tick
/// 65 or never ends it at 135. The constant is the fitted value the single
/// reference run supports; the release's mechanism is not recoverable from
/// its frames. [`Mntscrl::tree_wait`] keeps the tree's condition as the
/// measured counterfactual.
const RELEASE_WAIT_TICKS: u32 = 71;
/// `:3` `const veke = 2800` — dead, because `inc(frame)` is commented out at
/// `:221`. Kept as the source's number; no code path can use it.
#[cfg(test)]
const VEKE: u32 = 2800;

/// The reference boot index this part's tick 0 draws at, measured from the
/// fade-in's own recorded frame means (`docs/fidelity/reference-indices.md`
/// §11.1; `the_fade_in_pins_tick_zero_to_boot_419`). **Not an anchor**:
/// `shell::boot_anchor("mntscrl")` is `None` by decision, and a run of this
/// part numbers its own frames from zero. The number exists for one purpose:
/// the music's head start below, and the comparison in `docs/fidelity/mntscrl.md`.
const REFERENCE_FIRST_INDEX: u32 = 419;

/// `:153-159`'s music wait: the reference's player had been polled for the 11
/// frames between the music clock's start (408) and this part's tick 0 (419)
/// before the part ran. An unanchored run starts the player with the part, so
/// those polls are made here — the same `pre` a `MusicStart` anchored at 419
/// computes.
const MUSIC_PRE_ROLL: u32 = REFERENCE_FIRST_INDEX - crate::shell::MUSIC_START_FRAME;

// --------------------------------------------------------------- the part --

/// A palette the part mutates byte by byte, as the source's `array[0..768] of
/// byte` triples do. Six bits a channel; `set_entry` would mask, so the part
/// keeps the plain triples and builds a [`Palette`] only to upload.
type Rgb = [u8; 3];
type Palette768 = [Rgb; 256];

/// Where in `READ2.PAS`'s flow the part is. The wait's length is the music's,
/// so it is a phase and not a tick number; everything after it is fixed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// Tick 0: the mode set, the loads' results, the black DAC, the blit.
    Setup,
    /// `:132-142`: fade-in upload `y`.
    FadeIn { y: u32 },
    /// The release's first wait: `ticks` of its 71 have passed; nothing is
    /// drawn. The tree's `:153-159` `dis_musplus() >= 0` is the counterfactual
    /// [`Mntscrl::tree_wait`].
    Wait { ticks: u32 },
    /// `:165-182`: pre-roll iteration `y`.
    PreRoll { y: u32 },
    /// `:193-224`: the main loop.
    Main,
    /// The part has ended and keeps its last frame, as the loader does.
    Done,
}

/// MNTSCRL: the landscape, the font strip, and the three position lists.
pub struct Mntscrl {
    /// `hback`'s rebased memory: the 64,000-byte picture, then the asset's
    /// last 778 bytes where the rebase leaves them ([`BACKGROUND_LEN`]).
    background: Vec<u8>,
    /// `READ2.PAS:42` `fbuf`: the 31 loaded rows, the array's last byte, and
    /// the source's +128 bias applied to all of them.
    fbuf: Vec<u8>,
    /// `:43` `font : array[0..237*31] of byte`: one byte a blitter column,
    /// laid out 31 rows of 237.
    font: Vec<u8>,
    /// `posi1-3`, in `scr`'s 0, 1, 2 order.
    lists: Vec<PosList>,
    /// `:41` `pal`: the picture's own palette.
    pal: Palette768,
    /// `:44` `tmppal`: the fade's target.
    tmppal: Palette768,
    /// `:44` `fpal`: the DAC as last uploaded.
    fpal: Palette768,
    /// `:45` `scp`: the `O2.SCI` column the next `scr(2)` appends.
    scp: u16,
    /// The bound [`scp`](Self::scp) stops at — [`SCP_MAX`], the release's 640.
    /// A field so `the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639`
    /// can run the tree's 639 through the same code and measure the frames it
    /// produces; nothing outside that test sets it.
    scp_bound: u16,
    /// Run the tree's `:153-159` wait — `dis_musplus() >= 0` a tick — instead
    /// of the release's [`RELEASE_WAIT_TICKS`]. A field for the counterfactual
    /// test `the_trees_own_wait_would_hold_through_the_scroll_keyframes`,
    /// which measures what the development snapshot would have drawn; the
    /// part's own behaviour is the release's wait, and nothing at run time
    /// sets this.
    tree_wait: bool,
    /// `:45` `sss`: the list `scr` is on this tick.
    sss: u8,
    /// `:99` `fp`: the fade-out's step.
    fp: u8,
    /// `:98` `fadeout`: set by the cue and never cleared.
    fadeout: bool,
    /// `:95` `quit`: set on the fade-out's last tick.
    quit: bool,
    /// Where the part is.
    phase: Phase,
    /// The part's own tick, which is the shell's count for this part.
    tick: u32,
}

impl Mntscrl {
    /// A part with nothing loaded: `init` reads the three assets.
    pub fn new() -> Mntscrl {
        Mntscrl {
            background: Vec::new(),
            fbuf: Vec::new(),
            font: Vec::new(),
            lists: Vec::new(),
            pal: [[0; 3]; 256],
            tmppal: [[0; 3]; 256],
            fpal: [[0; 3]; 256],
            scp: SCP_START,
            scp_bound: SCP_MAX,
            tree_wait: false,
            sss: 0,
            fp: 0,
            fadeout: false,
            quit: false,
            phase: Phase::Setup,
            tick: 0,
        }
    }

    /// `:101-108`: mode 13h, `dis_partstart`, and the display state a mode set
    /// leaves: origin 0, 320 pixels a row, no split, overscan entry 0.
    fn set_display(ctx: &mut PartCtx) {
        ctx.view.width = SCREEN_WIDTH;
        ctx.view.height = SCREEN_HEIGHT;
        ctx.view.double_scan = false;
        ctx.view.set_origin(0);
        ctx.view.y_line_offset = Some(LINE_LEN);
        ctx.view.pan = 0;
        ctx.view.border = Some(0);
        ctx.view.split = None;
    }

    /// Tick 0: `:120-128` — the black DAC, the background blit, and the two
    /// fade tables. The mode set's clear is the surface's first 64 KB.
    fn setup(&mut self, ctx: &mut PartCtx) {
        // `:123` `move(mem[seg(hback):ofs(hback)],mem[$a000:0],64000)`. The
        // clear before it is the mode set's (`int10_modes.cpp`'s
        // `finish_set_mode`); the blit covers the whole display, so the clear
        // is only what the memory past the last written row needs.
        let memory = ctx.surface.pixels_mut();
        memory[..VIDEO_BYTES].fill(0);
        memory[..VIDEO_BYTES].copy_from_slice(&self.background[..VIDEO_BYTES]);

        // `:122` `for x := 0 to 255 do setrgb(x,0,0,0)`.
        ctx.set_palette(Palette::new());
    }

    /// `:132-142`: upload `fpal`, then step every channel toward `tmppal`.
    fn fade_in_tick(&mut self, ctx: &mut PartCtx) {
        self.upload_all(ctx);
        ramp(&mut self.fpal, &self.tmppal, 256, Direction::Up);
    }

    /// `:144-151`: `tmppal := pal`, `fpal := pal` with the two bands black, an
    /// upload, and the font build.
    fn finish_fade(&mut self, ctx: &mut PartCtx) {
        self.tmppal = self.pal;
        self.fpal = self.pal;
        black_bands(&mut self.fpal);
        self.upload_all(ctx);

        // `:150` `for x := 0 to 30 do move(fbuf[x*640],font[x*237+104],133)`.
        for row in 0..FBUF_ROWS {
            let source = row * O2_WIDTH;
            let destination = row * FONT_COLUMNS + FONT_OFFSET;
            self.font[destination..destination + FONT_FIRST_BYTES]
                .copy_from_slice(&self.fbuf[source..source + FONT_FIRST_BYTES]);
        }
    }

    /// `:165-182`: the pre-roll's iteration `y` — the band ramp on odd frames,
    /// then one `scr`.
    fn pre_roll_tick(&mut self, ctx: &mut PartCtx, y: u32) {
        if y % 2 == 1 {
            // `:171` `for x := 0 to 176 do setrgb(x,fpal…)`.
            for entry in 0..PRE_ROLL_ENTRIES {
                ctx.set_palette_entry(entry as u8, self.fpal[entry]);
            }
            // `:172-177`: the same entries step toward `tmppal`.
            ramp(
                &mut self.fpal,
                &self.tmppal,
                PRE_ROLL_ENTRIES,
                Direction::Up,
            );
        }
        self.scr(ctx, self.sss as usize);
        self.sss = if self.sss == 2 { 0 } else { self.sss + 1 };
    }

    /// `:193-224`: one pass of the main loop.
    fn main_tick(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `:196-203`: the cue is `== -11` exactly.
        let reading = ctx.musplus();
        if reading == CUE {
            self.fadeout = true;
        }
        if self.fadeout {
            // `:207` `if fp = 64 then quit := 1 else inc(fp)`.
            if self.fp == FADE_OUT_STEPS as u8 - 1 {
                self.quit = true;
            } else {
                self.fp += 1;
            }
            // `:208-214`: upload, then step every channel toward zero. The
            // upload is before the decrement, so the first fade tick still
            // shows the full palette.
            self.upload_all(ctx);
            ramp(&mut self.fpal, &[[0; 3]; 256], 256, Direction::Down);
        }

        // `:218-219`.
        self.scr(ctx, self.sss as usize);
        self.sss = if self.sss == 2 { 0 } else { self.sss + 1 };

        // `:224` `until dis_exit or (frame = veke) or (quit = 1)`. `frame` is
        // never incremented (`:221` is commented out), so the `veke` arm is
        // dead; `dis_exit` is the part's own check, after the tick's drawing.
        if self.quit || ctx.exit_requested {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }

    /// `:56-91` `scr(pos)` — one position list's columns into video memory.
    ///
    /// `Putrouts`'s `lodsw`/`or ax,ax`/`je` walk is the column loop; a zero
    /// count writes nothing and still consumes its count word (the decoder
    /// keeps it as an empty column). `scr(2)` shifts and appends afterwards,
    /// so the blit itself uses the font as it stands.
    fn scr(&mut self, ctx: &mut PartCtx, which: usize) {
        let memory = ctx.surface.pixels_mut();
        let list = &self.lists[which];
        for column in 0..FONT_LEN {
            let value = self.font[column];
            for &offset in list.column(column) {
                let at = offset as usize;
                memory[at] = self.background[at].wrapping_add(value);
            }
        }

        if which == 2 {
            // `:86` `move(font[1],font[0],237*31)`: 7,347 bytes, so the
            // packed array shifts one byte and the array's extra byte stays.
            self.font.copy_within(1..=FONT_LEN, 0);
            // `:87` `for ff := 0 to 30 do font[ff*237+236] := fbuf[ff*640+scp]`.
            for row in 0..FONT_ROWS {
                self.font[row * FONT_COLUMNS + (FONT_COLUMNS - 1)] =
                    self.fbuf[row * O2_WIDTH + self.scp as usize];
            }
            // `:88`, with the release's bound ([`SCP_MAX`]).
            if self.scp < self.scp_bound {
                self.scp += 1;
            }
        }
    }

    /// `setrgb(x,fpal[x*3+0],…)` for every entry: what `:135` and `:148`
    /// upload.
    fn upload_all(&self, ctx: &mut PartCtx) {
        let mut palette = Palette::new();
        for (index, rgb) in self.fpal.iter().enumerate() {
            palette.set_entry(index as u8, *rgb);
        }
        ctx.set_palette(palette);
    }
}

/// Parse and bound one POS list, `bytes` as read from `path`.
fn pos_list(path: &str, bytes: &[u8]) -> Result<PosList, PartError> {
    let list = PosList::parse(bytes).map_err(|error| PartError {
        part: NAME,
        message: format!("{path}: {error}"),
    })?;
    // `scr` indexes the surface and the background with every offset
    // (:515); past the 64,000-byte picture the background read panics.
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
    if list.columns.len() != FONT_LEN {
        return Err(PartError {
            part: NAME,
            message: format!(
                "{path} holds {} columns; the blitter walks {FONT_LEN} (237*31, \
                 FOREST/ROUTINES.ASM:21)",
                list.columns.len()
            ),
        });
    }
    Ok(list)
}

impl Part for Mntscrl {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `:110-151`: the assets and the display state the part needs before its
    /// first tick, and the music's head start.
    ///
    /// The source's mode set and loads all run between the previous part's
    /// last frame and this part's first, which is why the view is set here:
    /// the shell composes tick 0 at this geometry.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:153-159` is a music wait; a run with no player could never leave
        // it. The shell refuses to start such a run, and this is the
        // same refusal where the part can see it.
        ctx.require_music(
            NAME,
            "this part waits on the music player's position \
             (FOREST/READ2.PAS:153-159, :196-203) and the run has no music to wait on",
        )?;

        // The reference's player was 11 polls old when this part's tick 0 ran
        // (`REFERENCE_FIRST_INDEX` - `MUSIC_START_FRAME`); an unanchored entry
        // run starts it with the part, so the head start is made here. A part
        // reached mid-chain starts on a player that has already run through
        // these frames, so the head start is the entry run's
        // (`music_fresh`) alone.
        if ctx.music_fresh() {
            if let Some(music) = ctx.music() {
                for _ in 0..MUSIC_PRE_ROLL {
                    music.poll();
                }
            }
        }

        // `:110-117`: 31 rows of 640 bytes. The RIX3 reader hands back the
        // same bytes the raw `seek(x*640+778)` reaches (its own tests pin the
        // layout), so the copy is row for row.
        let o2 = load_image(NAME, O2_PATH)?;
        if (o2.width() as usize, o2.height() as usize) != (O2_WIDTH, O2_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{O2_PATH} is {}x{}; the part reads {O2_WIDTH}x{O2_HEIGHT}",
                    o2.width(),
                    o2.height()
                ),
            });
        }
        self.fbuf = vec![0; FBUF_LEN];
        self.fbuf[..FBUF_LOADED].copy_from_slice(&o2.pixels()[..FBUF_LOADED]);
        // `:119`: the bias, including the loop's one-byte overrun.
        let mut overrun = 0u8;
        apply_bias(&mut self.fbuf, &mut overrun);

        // `:120-121`: the palette at +10 and the picture at +778.
        let hillback = load_image(NAME, HILLBACK_PATH)?;
        if (hillback.width() as usize, hillback.height() as usize)
            != (HILLBACK_WIDTH, HILLBACK_HEIGHT)
        {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{HILLBACK_PATH} is {}x{}; the part reads {HILLBACK_WIDTH}x{HILLBACK_HEIGHT}",
                    hillback.width(),
                    hillback.height()
                ),
            });
        }
        self.pal = core::array::from_fn(|index| hillback.palette.entry(index as u8));
        let mut background = Vec::with_capacity(BACKGROUND_LEN);
        background.extend_from_slice(&hillback.pixels()[..VIDEO_BYTES]);
        self.background = background;
        // The rebase leaves the asset's own last 778 bytes where the segment
        // held them, so they are the picture's last 778.
        self.background
            .extend_from_slice(&hillback.pixels()[VIDEO_BYTES - 778..VIDEO_BYTES]);

        // `:59-90`: the three lists, one frame group each.
        self.lists = Vec::with_capacity(3);
        for rel in POS_PATHS {
            let bytes = read_asset(NAME, rel)?;
            self.lists.push(pos_list(&asset(rel), &bytes)?);
        }

        self.font = vec![0; FONT_LEN + 1];
        self.scp = SCP_START;
        self.scp_bound = SCP_MAX;
        self.tree_wait = false;
        self.sss = 0;
        self.fp = 0;
        self.fadeout = false;
        self.quit = false;
        self.phase = Phase::Setup;
        self.tick = 0;

        // `:125-128`: the fade's first target is the picture's palette with
        // the two bands black, and the DAC starts at zero.
        self.tmppal = self.pal;
        black_bands(&mut self.tmppal);
        self.fpal = [[0; 3]; 256];

        // The part addresses 64 KB of video memory flat, so the surface has to
        // hold it: this is the engine's own surface size (640x400 = 256,000
        // pixels), and a smaller one would put the blitter's writes out of
        // bounds rather than clip them.
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

        // `:101-108`.
        Mntscrl::set_display(ctx);
        Ok(())
    }

    /// One tick of `READ2.PAS:132-224`. See the module docs' tick map.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick += 1;
        match self.phase {
            Phase::Done => PartStatus::Finished,
            Phase::Setup => {
                self.setup(ctx);
                self.phase = Phase::FadeIn { y: 0 };
                PartStatus::Continuing
            }
            Phase::FadeIn { y } => {
                self.fade_in_tick(ctx);
                if y + 1 == FADE_IN_STEPS {
                    self.finish_fade(ctx);
                    self.phase = Phase::Wait { ticks: 0 };
                } else {
                    self.phase = Phase::FadeIn { y: y + 1 };
                }
                PartStatus::Continuing
            }
            Phase::Wait { ticks } => {
                // The source's loop never checks `dis_exit`, but the port does
                // not spin: an exit request ends the wait on the tick it is
                // seen (the phase's Review Focus 1).
                if ctx.exit_requested {
                    self.phase = Phase::Done;
                    return PartStatus::Finished;
                }
                if self.tree_wait {
                    // The development snapshot's `:156-158` `cmp dx,0 / jl`.
                    let reading = ctx.musplus();
                    if reading >= 0 {
                        self.phase = Phase::PreRoll { y: 0 };
                        return PartStatus::Continuing;
                    }
                } else if ticks + 1 >= RELEASE_WAIT_TICKS {
                    // The release's hold is up; the next tick is the
                    // pre-roll's first `waitr` (boot 555 for tick 0 at 419).
                    self.phase = Phase::PreRoll { y: 0 };
                    return PartStatus::Continuing;
                }
                self.phase = Phase::Wait { ticks: ticks + 1 };
                PartStatus::Continuing
            }
            Phase::PreRoll { y } => {
                self.pre_roll_tick(ctx, y);
                if y + 1 == PRE_ROLL_STEPS {
                    // `:184` `fillchar(tmppal,768,#0)`: the fade-out's target.
                    self.tmppal = [[0; 3]; 256];
                    self.phase = Phase::Main;
                } else {
                    self.phase = Phase::PreRoll { y: y + 1 };
                }
                PartStatus::Continuing
            }
            Phase::Main => {
                let status = self.main_tick(ctx);
                if status == PartStatus::Finished {
                    self.phase = Phase::Done;
                }
                status
            }
        }
    }
}

/// `:119` — the +128 bias on every non-zero byte, over the source's loop.
///
/// The bound is `sizeof(fbuf)`, and `fbuf` is `array[0..640*31]`: 19,841
/// elements, so `x` runs 0 through **19,841** — one past the array's last
/// index, 19,840. In the original's data segment the next byte is `font[0]`
/// (the next global declared, `:43`), still zero at this point because the
/// font is built at `:150`; the increment is therefore a no-op, but the read
/// is real and the port keeps it as the explicit `overrun` byte.
fn apply_bias(buffer: &mut [u8], overrun: &mut u8) {
    for value in buffer.iter_mut().chain(core::iter::once(overrun)) {
        if *value > 0 {
            // `inc` on a byte: 255 wraps to 0, which no `O2.SCI` byte reaches.
            *value = value.wrapping_add(BIAS);
        }
    }
}

/// The two forced-black bands (`:126-127`, `:146-147`).
fn black_bands(palette: &mut Palette768) {
    for (first, end) in BLACK_BANDS {
        for entry in first..end {
            palette[usize::from(entry)] = [0, 0, 0];
        }
    }
}

/// One ramp step, channel by channel: `if fpal < tmppal then inc` (`:138-140`)
/// or `if fpal > tmppal then dec` (`:211-213`). `count` is 256 for the two
/// full uploads and 177 for the pre-roll.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Direction {
    Up,
    Down,
}

fn ramp(from: &mut Palette768, to: &Palette768, count: usize, direction: Direction) {
    for entry in 0..count {
        for channel in 0..3 {
            match direction {
                Direction::Up if from[entry][channel] < to[entry][channel] => {
                    from[entry][channel] += 1;
                }
                Direction::Down if from[entry][channel] > to[entry][channel] => {
                    from[entry][channel] -= 1;
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::{differing_bytes, reference, Reading, Rig, ScriptedMusic};
    use sr_engine::dac_expand;
    use sr_engine::music::musplus_from;

    /// A script with no plus pending: `musplus` is -32 at every tick (the
    /// object's own sentinel), so the main loop's -11 cue never comes.
    fn quiet_script(last_tick: u32) -> Vec<(i32, i32)> {
        vec![(0, 0); last_tick as usize + 1]
    }

    /// The tree's counterfactual: `musplus` is -32 until the trace's first
    /// non-negative reading, order 47 row 0 at trace frame 2488, which the
    /// port's one-frame lead puts on tick 2068; from there it is 0.
    fn tree_script(last_tick: u32) -> Vec<(i32, i32)> {
        (0..=last_tick)
            .map(|tick| if tick < 2068 { (0, 0) } else { (2, 0) })
            .collect()
    }

    /// A player answering `musplus_from(zplus, row)` from `script`, one
    /// `(zplus, row)` a tick; a tick past the end reads the no-plus sentinel.
    fn scripted(script: Vec<(i32, i32)>) -> ScriptedMusic {
        ScriptedMusic::musplus(script.into_iter().map(|(zplus, row)| musplus_from(zplus, row)).collect())
            .past_end(Reading::default())
            .never_syncs("`READ2.PAS` never syncs: its music call is `dis_musplus()`")
    }

    /// A player with no plus pending: `musplus()` is -32 at every tick.
    fn sentinel() -> ScriptedMusic {
        scripted(Vec::new())
    }

    fn rig(music: ScriptedMusic) -> Rig {
        Rig::new(SCREEN_WIDTH, SCREEN_HEIGHT).with(music)
    }

    fn started_with(music: ScriptedMusic) -> (Rig, Mntscrl) {
        let mut rig = rig(music);
        let mut part = Mntscrl::new();
        part.init(&mut rig.init_ctx(0)).unwrap();
        (rig, part)
    }

    fn started() -> (Rig, Mntscrl) {
        started_with(sentinel())
    }

    /// Runs the part for `ticks` further ticks, numbered by its own count.
    fn run_ticks(rig: &mut Rig, part: &mut Mntscrl, ticks: u32) {
        for _ in 0..ticks {
            let at = part.tick;
            assert_eq!(
                part.frame(&mut rig.ctx(at)),
                PartStatus::Continuing,
                "tick {at}"
            );
        }
    }

    /// Runs to (and not including) `tick`, numbered by the part's own count.
    fn run_to(rig: &mut Rig, part: &mut Mntscrl, tick: u32) {
        while part.tick < tick {
            run_ticks(rig, part, 1);
        }
    }

    /// The live palette as the port will upload it, one 6-bit triple a entry.
    fn live_palette(rig: &Rig) -> Palette768 {
        core::array::from_fn(|index| rig.palette.entry(index as u8))
    }

    // --- the assets ---------------------------------------------------------

    #[test]
    fn the_assets_are_read_the_way_the_source_reads_them() {
        let (_, part) = started();

        // `:110-117`: 31 rows of 640 bytes from `O2.SCI`'s pixel payload
        // (RIX3 puts it at 778, where the raw `seek(x*640+778)` looks), and
        // `:119`'s +128 on every non-zero byte.
        let o2 = read_asset(NAME, O2_PATH).unwrap();
        assert_eq!(o2.len(), 21_258);
        for (path, length) in POS_PATHS.iter().zip([25_088usize, 25_064, 25_114]) {
            assert_eq!(
                std::fs::metadata(asset(path)).unwrap().len() as usize,
                length,
                "{path}"
            );
        }
        for row in 0..FBUF_ROWS {
            for column in 0..O2_WIDTH {
                let raw = o2[778 + row * O2_WIDTH + column];
                let expected = if raw > 0 { raw.wrapping_add(BIAS) } else { 0 };
                assert_eq!(
                    part.fbuf[row * O2_WIDTH + column],
                    expected,
                    "({column}, {row})"
                );
            }
        }
        // The array's last element is never loaded and stays zero.
        assert_eq!(part.fbuf.len(), FBUF_LEN);
        assert_eq!(part.fbuf[FBUF_LOADED], 0);

        // `:120-121`: the palette at +10, the picture at +778 — and the
        // rebased segment's trailing 778 bytes are the picture's own last 778.
        let hillback = read_asset(NAME, HILLBACK_PATH).unwrap();
        assert_eq!(hillback.len(), 64_778);
        for index in 0..256usize {
            let expected = [
                hillback[RIX3_PALETTE + index * 3],
                hillback[RIX3_PALETTE + index * 3 + 1],
                hillback[RIX3_PALETTE + index * 3 + 2],
            ];
            assert_eq!(part.pal[index], expected, "entry {index}");
        }
        assert_eq!(part.background.len(), BACKGROUND_LEN);
        assert_eq!(
            &part.background[..VIDEO_BYTES],
            &hillback[778..778 + VIDEO_BYTES],
            "the first 64,000 bytes are the picture"
        );
        assert_eq!(
            &part.background[VIDEO_BYTES..],
            &hillback[778 + VIDEO_BYTES - 778..778 + VIDEO_BYTES],
            "the rebase leaves the asset's last 778 bytes standing"
        );

        // `:59-90`: three lists of `237*31` columns. The offsets are 16-bit
        // video addresses and the largest is inside the picture.
        assert_eq!(part.lists.len(), 3);
        let mut positions = 0usize;
        let mut max_offset = 0u16;
        for (index, list) in part.lists.iter().enumerate() {
            assert_eq!(list.columns.len(), FONT_LEN, "list {index}");
            for column in &list.columns {
                positions += column.len();
                if let Some(offset) = column.iter().max() {
                    max_offset = max_offset.max(*offset);
                }
            }
        }
        assert_eq!(positions, 15_592, "POS1-3's positions, the survey's count");
        assert_eq!(max_offset, 63_870, "the largest destination offset");
        assert!((max_offset as usize) < VIDEO_BYTES);
    }

    #[test]
    fn an_offset_past_the_picture_is_refused_not_indexed() {
        // `scr` indexes the surface and the background with each column's
        // offsets (:515): the picture is `VIDEO_BYTES` = 64,000 bytes, so a
        // 65,000 offset would panic there rather than draw. The file carries
        // the blitter's full column count too, or the count check would
        // answer instead of the offset bound.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&1u16.to_le_bytes()); // column 0: one offset
        bytes.extend_from_slice(&65_000u16.to_le_bytes());
        for _ in 1..FONT_LEN {
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
    fn the_bias_includes_the_loops_one_byte_overrun() {
        // `:119` `for x := 0 to sizeof(fbuf)`: 19,841 is the array's last
        // index, so one more byte — `font[0]` in the original's data segment —
        // is read and incremented too. The port's extra byte is explicit.
        let mut buffer = [1u8, 0, 255];
        let mut overrun = 7u8;
        apply_bias(&mut buffer, &mut overrun);
        assert_eq!(
            buffer,
            [129, 0, 127],
            "the +128 wraps at 255, as `inc` does"
        );
        assert_eq!(overrun, 135, "the byte past the array is in the loop too");

        // And the real array ends with a zero overrun: the original's next
        // global is the font, which `:150` does not build until later.
        let (_, part) = started();
        let mut copied = part.fbuf.clone();
        let mut overrun = 0u8;
        apply_bias(&mut copied, &mut overrun);
        assert_eq!(overrun, 0);
    }

    // --- the tick map -------------------------------------------------------

    #[test]
    fn the_fade_in_pins_tick_zero_to_boot_419() {
        // The reference's own recorded frame means (`reference-indices.md`
        // §11.1: 425 → 6.08, 430 → 8.91, 435 → 10.25, 440 → 10.53) against the
        // port's palette curve. At fade step `n` the DAC is `min(n, tmppal)`
        // per channel, the screen is the background alone, and the mean is
        // over the 64,000 pixels. The four means land on steps 5, 10, 15 and
        // 20 and the frame a step is displayed on is `tick 0 + n + 1`, so all
        // four say tick 0 is 419.
        let hillback = load_image(NAME, HILLBACK_PATH).unwrap();
        let palette: Palette768 = core::array::from_fn(|i| hillback.palette.entry(i as u8));
        let mut target = palette;
        black_bands(&mut target);

        let mean_at = |step: u8| -> f64 {
            let mut total = 0u64;
            for &index in hillback.pixels() {
                let shown: Rgb = core::array::from_fn(|c| target[index as usize][c].min(step));
                total += shown.iter().map(|v| u64::from(dac_expand(*v))).sum::<u64>();
            }
            total as f64 / (hillback.pixels().len() as f64 * 3.0)
        };
        let means: Vec<f64> = (0..=63).map(mean_at).collect();
        for (frame, recorded) in [(425u32, 6.08f64), (430, 8.91), (435, 10.25), (440, 10.53)] {
            let step = (0..=63)
                .min_by(|a, b| {
                    (means[*a] - recorded)
                        .abs()
                        .total_cmp(&(means[*b] - recorded).abs())
                })
                .unwrap();
            assert!(
                (means[step] - recorded).abs() < 0.005,
                "frame {frame}: the model's best step {step} is {:.4}, not {recorded}",
                means[step]
            );
            assert_eq!(
                frame - step as u32 - 1,
                REFERENCE_FIRST_INDEX,
                "frame {frame} at step {step}"
            );
        }
        assert_eq!(REFERENCE_FIRST_INDEX, 419);
    }

    #[test]
    fn tick_zero_is_the_background_through_a_black_dac() {
        let (mut rig, mut part) = started();
        // `init` polls the player 11 times and draws nothing; tick 0 is the
        // setup and the background blit.
        assert_eq!(rig.player().polls, MUSIC_PRE_ROLL, "the music's head start");
        assert_eq!(MUSIC_PRE_ROLL, 11);
        assert_eq!(part.frame(&mut rig.ctx(0)), PartStatus::Continuing);

        assert_eq!(
            &rig.surface.pixels()[..VIDEO_BYTES],
            &part.background[..VIDEO_BYTES],
            "`:123`'s 64,000-byte blit"
        );
        assert!(
            (0..256).all(|index| rig.palette.entry(index as u8) == [0, 0, 0]),
            "`:122` sets every DAC entry black"
        );
        // `:101-108`: mode 13h, at the surface's first 64 KB.
        assert_eq!(
            (rig.view.width, rig.view.height),
            (SCREEN_WIDTH, SCREEN_HEIGHT)
        );
        assert_eq!(rig.view.y_line_offset, Some(LINE_LEN));
        assert_eq!(rig.view.start_addr, 0);
        assert!(!rig.view.double_scan);
        assert!(rig.view.split.is_none());
    }

    #[test]
    fn a_mid_chain_start_does_not_pre_roll_the_music() {
        // A part the block's chain reaches is handed a player that has
        // already run through the 11 frames between the music clock's start
        // and this part's tick 0. The head start is the entry run's
        // (`music_fresh`) alone.
        let mut rig = rig(sentinel());
        let mut part = Mntscrl::new();
        part.init(&mut rig.init_ctx_chain(0)).unwrap();
        assert_eq!(rig.player().polls, 0, "a mid-chain start polls nothing");
    }

    #[test]
    fn the_fade_in_is_sixty_four_uploads_one_step_a_tick() {
        let (mut rig, mut part) = started();
        part.frame(&mut rig.ctx(0)); // setup

        // Tick 1 uploads `fpal` as it stands — zero.
        part.frame(&mut rig.ctx(1));
        assert!(live_palette(&rig).iter().all(|rgb| *rgb == [0, 0, 0]));
        // Tick 2 uploads the first increment: every channel that targets more
        // than zero is 1; the forced-black bands are still 0.
        part.frame(&mut rig.ctx(2));
        let live = live_palette(&rig);
        assert_eq!(live[32], [1, 1, 1], "entry 32's target is (8, 6, 6)");
        assert_eq!(live[1], [0, 0, 0], "entry 1 is inside the first black band");
        assert_eq!(live[151], [0, 0, 0], "entry 151 is inside the second");

        run_to(&mut rig, &mut part, FADE_IN_STEPS + 1);
        let live = live_palette(&rig);
        let expected = part.pal; // `pal` itself, with the bands blackened
        let mut expected = expected;
        black_bands(&mut expected);
        assert_eq!(live, expected, "`:144-148` after the 64th upload");
        assert_eq!(part.phase, Phase::Wait { ticks: 0 });
        // `:150-151`: the font and `scp`.
        for row in 0..FBUF_ROWS {
            assert_eq!(
                &part.font[row * FONT_COLUMNS + FONT_OFFSET..][..FONT_FIRST_BYTES],
                &part.fbuf[row * O2_WIDTH..][..FONT_FIRST_BYTES],
                "font row {row}"
            );
            assert!(
                part.font[row * FONT_COLUMNS..][..FONT_OFFSET]
                    .iter()
                    .all(|byte| *byte == 0),
                "columns 0-103 of row {row} stay transparent"
            );
        }
        assert_eq!(part.scp, SCP_START);
        assert_eq!(part.sss, 0);
    }

    #[test]
    fn the_wait_is_the_releases_seventy_one_ticks() {
        // The release's own frames put its last held frame at boot 554 and its
        // pre-roll's first `scr` at 555; with tick 0 at 419 that is ticks 65
        // through 135 — **71 ticks** — and the pre-roll's first `waitr` is
        // tick 136. The wait draws nothing and its length is the release's,
        // not the music's: the trace's position is order 42 row 16 on both
        // sides of the boundary (`zplus` 0, `musplus` -32 at both), so no
        // `musplus` threshold can end it there.
        assert_eq!(RELEASE_WAIT_TICKS, 71);
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, FADE_IN_STEPS + 1); // tick 65
        let surface = rig.surface.pixels().to_vec();
        let palette = live_palette(&rig);
        let view = rig.view.clone();
        run_ticks(&mut rig, &mut part, 10); // ticks 65..=74
        assert_eq!(part.phase, Phase::Wait { ticks: 10 });
        assert_eq!(rig.surface.pixels(), &surface[..], "the wait drew");
        assert_eq!(live_palette(&rig), palette, "the wait moved the DAC");
        assert_eq!(rig.view, view, "the wait moved the display");

        // A -11 reading inside the wait changes nothing: the cue belongs to
        // the main loop, and the wait's length is the release's.
        let mut script = quiet_script(200);
        script[70] = (1, 53); // musplus -11
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, 100);
        assert_eq!(part.phase, Phase::Wait { ticks: 35 });
        assert!(!part.fadeout, "the wait does not fade on -11");
        assert_eq!(rig.surface.pixels(), &surface[..], "the wait drew");

        // The boundary: tick 134 is the 70th wait tick and still holds; tick
        // 135 is the 71st and turns the phase over; the pre-roll's first
        // `scr` is tick 136.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 135); // through tick 134
        assert_eq!(part.phase, Phase::Wait { ticks: 70 });
        assert_eq!(rig.surface.pixels(), &surface[..]);
        run_to(&mut rig, &mut part, 136); // through tick 135
        assert_eq!(part.phase, Phase::PreRoll { y: 0 });
        assert_eq!(
            rig.surface.pixels(),
            &surface[..],
            "the wait's last tick drew nothing"
        );
        part.frame(&mut rig.ctx(136));
        assert_ne!(
            rig.surface.pixels(),
            &surface[..],
            "the pre-roll's first scr"
        );
        assert_eq!(part.sss, 1);
    }

    #[test]
    fn exit_requested_ends_the_wait_on_that_tick() {
        // (c) The source's wait never checks `dis_exit`; the port does not
        // spin, so an exit notice is honoured on the tick it arrives.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, FADE_IN_STEPS + 5);
        assert_eq!(part.phase, Phase::Wait { ticks: 4 });
        let surface = rig.surface.pixels().to_vec();
        assert_eq!(
            part.frame(&mut rig.ctx_exit(part.tick)),
            PartStatus::Finished
        );
        assert_eq!(part.phase, Phase::Done);
        assert_eq!(rig.surface.pixels(), &surface[..], "the exit tick drew");
    }

    #[test]
    fn the_pre_roll_uploads_on_odd_frames_and_ramps_entries_zero_to_176() {
        // `:165-182`: 127 iterations, the band ramp on odd `y` only, `scr`
        // every tick with `sss` cycling.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 136); // through the wait's last tick 135: nothing drawn
        assert_eq!(part.phase, Phase::PreRoll { y: 0 });
        let after_fade = live_palette(&rig);

        // y = 0 (tick 136): no upload.
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        assert_eq!(live_palette(&rig), after_fade);
        // y = 1: upload the step-0 table (still black in the bands), then
        // increment — the live DAC does not move yet.
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        assert_eq!(live_palette(&rig), after_fade);
        // y = 2: no upload.
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        assert_eq!(live_palette(&rig), after_fade);
        // y = 3: upload step 1.
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        let live = live_palette(&rig);
        assert_eq!(live[1], [1, 1, 1], "entry 1's first step");
        assert_eq!(
            live[32], part.pal[32],
            "entries outside the bands never move"
        );
        assert_eq!(
            live[200], part.pal[200],
            "entries past 176 are not uploaded"
        );

        // The loop's end: `scp` has advanced one per `scr(2)` — 42 of them —
        // and the next `sss` is 1.
        run_to(&mut rig, &mut part, 263);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(part.scp, SCP_START + 42);
        assert_eq!(part.sss, 1);
        assert_eq!(
            part.tmppal, [[0; 3]; 256],
            "`:184` zeroes the fade's target"
        );
        // Entry 151's blue channel targets 63 and the last odd iteration
        // uploaded 62, so the live DAC shows 62 — the source's own one-step
        // shortfall, kept.
        assert_eq!(part.pal[151], [57, 54, 63]);
        assert_eq!(live_palette(&rig)[151], [57, 54, 62]);
        assert_eq!(live_palette(&rig)[150], part.pal[150]);
    }

    // --- the blitter --------------------------------------------------------

    /// A part with synthetic assets: every column empty but three, so `scr`
    /// can be read one byte at a time.
    fn synthetic() -> Mntscrl {
        let mut columns = vec![Vec::new(); FONT_LEN];
        columns[0] = vec![100, 200, 300];
        columns[1] = Vec::new(); // a zero count: the hidden column
        columns[2] = vec![400];
        columns[3] = vec![500];
        let lists = (0..3)
            .map(|_| PosList {
                columns: columns.clone(),
            })
            .collect();
        let mut part = Mntscrl::new();
        part.background = vec![10; BACKGROUND_LEN];
        part.font = vec![0; FONT_LEN + 1];
        part.fbuf = (0..FBUF_LEN).map(|i| (i % 251) as u8).collect();
        part.lists = lists;
        part.scp = 133;
        part
    }

    #[test]
    fn scr_adds_the_font_column_to_the_background_and_a_zero_count_is_transparent() {
        let mut rig = rig(sentinel());
        let mut part = synthetic();
        part.font[0] = 7;
        part.font[2] = 9;
        part.font[3] = 0;
        part.scr(&mut rig.ctx(0), 0);
        assert_eq!(rig.surface.pixels()[100], 17, "background 10 + font 7");
        assert_eq!(rig.surface.pixels()[200], 17);
        assert_eq!(rig.surface.pixels()[300], 17);
        assert_eq!(rig.surface.pixels()[400], 19, "font column 2 is 9");
        assert_eq!(
            rig.surface.pixels()[500],
            10,
            "font column 3 is 0: the background shows through"
        );
        // Column 1's count is zero, so nothing it might have held is written.
        assert_eq!(
            rig.surface.pixels()[..100]
                .iter()
                .filter(|p| **p != 0)
                .count(),
            0
        );
    }

    #[test]
    fn scr_two_shifts_the_font_and_appends_the_next_o2_column() {
        let mut rig = rig(sentinel());
        let mut part = synthetic();
        for row in 0..FONT_ROWS {
            part.font[row * FONT_COLUMNS + 10] = 40 + row as u8;
            part.font[row * FONT_COLUMNS + FONT_COLUMNS - 1] = 99;
        }
        let appended = |part: &Mntscrl, row: usize| part.fbuf[row * O2_WIDTH + 133];
        part.scr(&mut rig.ctx(0), 2);
        assert_eq!(part.scp, 134, "`:88`");
        for row in 0..FONT_ROWS {
            assert_eq!(
                part.font[row * FONT_COLUMNS + 9],
                40 + row as u8,
                "row {row} shifted one byte left"
            );
            assert_eq!(
                part.font[row * FONT_COLUMNS + FONT_COLUMNS - 1],
                appended(&part, row),
                "row {row} got the new O2 column at 236"
            );
        }
        // `scp` stops at the bound: the append at 640 reads the packed
        // array one column on, which is the next row's first byte (and, for
        // row 30, the array's own extra element).
        part.scp = SCP_MAX;
        part.scr(&mut rig.ctx(0), 2);
        assert_eq!(part.scp, SCP_MAX);
        for row in 0..FONT_ROWS {
            assert_eq!(
                part.font[row * FONT_COLUMNS + FONT_COLUMNS - 1],
                part.fbuf[row * O2_WIDTH + SCP_MAX as usize],
                "row {row} at the bound"
            );
        }
    }

    // --- the cue and the fade-out ------------------------------------------

    /// The tick the main loop starts on: 64 fade ticks, the release's 71-tick
    /// wait and 127 pre-roll ticks — 263, which is boot 682.
    const MAIN_FIRST: u32 = 263;

    #[test]
    fn the_cue_is_exactly_minus_eleven() {
        // `:200` `cmp dx,-11 / jnz @a`. -10 and -12 must both leave the part
        // scrolling with the full palette; -11 starts the fade.
        let mut script = quiet_script(300);
        script[MAIN_FIRST as usize] = (1, 54); // -10
        script[MAIN_FIRST as usize + 1] = (1, 52); // -12
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, MAIN_FIRST);
        assert_eq!(part.phase, Phase::Main);
        let full = live_palette(&rig);
        run_ticks(&mut rig, &mut part, 10);
        assert!(!part.fadeout, "-10 and -12 are not the cue");
        assert_eq!(live_palette(&rig), full, "no fade without the cue");

        // And -11 is: the fade's first upload is still the full palette.
        let mut script = quiet_script(400);
        script[MAIN_FIRST as usize] = (1, 53); // -11
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, MAIN_FIRST);
        // The pre-roll's last upload was its 62nd step; `fpal` itself went one
        // further, and the fade's first upload is `fpal` (`:208-210`) — so the
        // first fade tick is brighter than the scrolling screen by that step.
        let fpal = part.fpal;
        part.frame(&mut rig.ctx(MAIN_FIRST));
        assert!(part.fadeout);
        assert_eq!(part.fp, 1);
        assert_eq!(live_palette(&rig), fpal, "uploaded before the decrement");
    }

    #[test]
    fn the_fade_out_is_sixty_five_ticks_and_ends_the_part() {
        let mut script = quiet_script(400);
        script[MAIN_FIRST as usize] = (1, 53);
        let (mut rig, mut part) = started_with(scripted(script));
        run_to(&mut rig, &mut part, MAIN_FIRST);
        assert_eq!(part.phase, Phase::Main);
        let full = part.fpal;

        // Tick 1 of the fade uploads `fpal` (the pre-roll's target, which the
        // live DAC has not shown yet), then steps once.
        assert_eq!(part.frame(&mut rig.ctx(MAIN_FIRST)), PartStatus::Continuing);
        assert_eq!(live_palette(&rig), full);
        // Tick 2 shows one step down.
        assert_eq!(
            part.frame(&mut rig.ctx(MAIN_FIRST + 1)),
            PartStatus::Continuing
        );
        let stepped = live_palette(&rig);
        let mut expected = full;
        ramp(&mut expected, &[[0; 3]; 256], 256, Direction::Down);
        assert_eq!(stepped, expected);

        // Ticks 3..=64 leave one step a tick; the 65th uploads black and ends
        // the loop (`:207`, `:224`).
        run_ticks(&mut rig, &mut part, FADE_OUT_STEPS - 3);
        assert_eq!(part.fp, 64);
        assert_eq!(
            part.frame(&mut rig.ctx(MAIN_FIRST + FADE_OUT_STEPS - 1)),
            PartStatus::Finished
        );
        assert!(part.quit);
        assert_eq!(part.phase, Phase::Done);
        assert!(
            (0..256).all(|index| rig.palette.entry(index as u8) == [0, 0, 0]),
            "the last fade tick uploads all zeros"
        );
        // And the part keeps returning `Finished`.
        assert_eq!(part.frame(&mut rig.ctx(9999)), PartStatus::Finished);
    }

    #[test]
    fn the_dead_veke_bound_never_ends_the_part() {
        // `:3` `veke = 2800`, `:221`'s `inc(frame)` commented out, `:224`'s
        // `frame = veke` therefore never true. The script never reaches -11.
        assert_eq!(VEKE, 2800);
        let (mut rig, mut part) =
            started_with(scripted(quiet_script(MAIN_FIRST + VEKE + 2)));
        run_to(&mut rig, &mut part, MAIN_FIRST + VEKE + 1);
        assert_eq!(part.phase, Phase::Main);
        assert_eq!(part.frame(&mut rig.ctx(part.tick)), PartStatus::Continuing);
        assert!(!part.fadeout);
    }

    // --- the reference ------------------------------------------------------

    #[test]
    fn the_held_opening_is_the_references_frame_500() {
        // `reference-indices.md` §11.2: 500 is the hold the part's music wait
        // is. Boot 500 is tick 81 (`REFERENCE_FIRST_INDEX`); the screen is the
        // background with the two bands' DAC entries black.
        let (mut rig, mut part) = started();
        run_to(&mut rig, &mut part, 500 - REFERENCE_FIRST_INDEX + 1);
        let indices = rig.rendered_indices();
        assert_eq!(
            differing_bytes(&indices, &reference("mntscrl", 500, "idx")),
            0,
            "500's index plane"
        );
        let rgb = rig.rendered_rgb();
        assert_eq!(
            differing_bytes(&rgb, &reference("mntscrl", 500, "rgb")),
            0,
            "500's RGB"
        );
    }

    #[test]
    fn the_four_keyframes_match_the_reference_at_the_stated_mapping() {
        // The part's own timeline is the release's: tick 0 is boot 419, the
        // first wait is 71 ticks and the pre-roll 127, so the main loop starts
        // at tick 263 (boot 682). The reference's frame at boot `K` is the
        // call at tick `K - 419`, and all four keyframes compare byte for
        // byte. The frame either side of each is a different picture (the
        // scroller moves 2.4-3.1 % of its pixels a frame), which is what makes
        // the mapping exact and not a search's loosest fit.
        let tick = |index: u32| index - REFERENCE_FIRST_INDEX;

        // 500: both planes. It is inside the wait, so it is the held opening.
        {
            let (mut rig, mut part) = started();
            run_to(&mut rig, &mut part, tick(500) + 1);
            assert_eq!(
                differing_bytes(&rig.rendered_indices(), &reference("mntscrl", 500, "idx")),
                0,
                "500's index plane"
            );
            assert_eq!(
                differing_bytes(&rig.rendered_rgb(), &reference("mntscrl", 500, "rgb")),
                0,
                "500's RGB"
            );
        }

        // The three scroll keyframes, RGB-only in the reference, with D19's
        // support: the neighbours are distinguishable from the keyframe.
        for index in [1100u32, 1700, 2300] {
            let frame = |at: u32| -> Vec<u8> {
                let (mut rig, mut part) = started();
                run_to(&mut rig, &mut part, at + 1);
                rig.rendered_rgb()
            };
            let key = frame(tick(index));
            assert_eq!(
                differing_bytes(&key, &reference("mntscrl", index, "rgb")),
                0,
                "boot {index} (tick {})",
                tick(index)
            );
            for neighbour in [tick(index) - 1, tick(index) + 1] {
                assert_ne!(
                    frame(neighbour),
                    key,
                    "boot {index}: tick {neighbour} is not distinguishable"
                );
            }
        }
    }

    #[test]
    fn the_releases_scp_bound_is_sixty_forty_where_the_tree_says_639() {
        // `READ2.PAS:88` in the tree is `if scp < 639 then inc(scp)`, so the
        // tree's last appended column is 639. The release's frame at boot
        // 2300 is byte-identical only under 640; under 639 it differs in 401
        // bytes (146 pixels), all of them in the columns the bound has
        // reached. 1100 and 1700 are before the bound, so they cannot tell
        // the two apart — which is what makes the 2300 difference the
        // bound's and not something else's.
        assert_eq!(SCP_MAX, 640);
        let render = |bound: u16, index: u32| -> Vec<u8> {
            let tick = index - REFERENCE_FIRST_INDEX;
            let (mut rig, mut part) = started();
            part.scp_bound = bound;
            run_to(&mut rig, &mut part, tick + 1);
            rig.rendered_rgb()
        };
        for index in [1100u32, 1700] {
            let tree = render(639, index);
            let release = render(640, index);
            assert_eq!(tree, release, "boot {index} is before the bound");
            assert_eq!(differing_bytes(&release, &reference("mntscrl", index, "rgb")), 0);
        }

        // The cap's own tick, so the note's number is test-produced: with the
        // release's 71-tick wait the main loop starts at tick 263 and `scr(2)`
        // falls on main ticks 1, 4, 7, …; `scp` starts at 133 and reaches 640
        // on the 507th shift, which is tick 1,656. Tick 1,655 still reads
        // 639; 1,656 reads 640 and appends the next row's first column.
        {
            let (mut rig, mut part) = started();
            run_to(&mut rig, &mut part, 1_655 + 1); // through tick 1,655
            assert_eq!(part.scp, 639, "the 506th shift");
            run_to(&mut rig, &mut part, 1_656 + 1); // through tick 1,656
            assert_eq!(part.scp, 640, "the 507th shift");
        }

        let target = reference("mntscrl", 2300, "rgb");
        let tree = render(639, 2300);
        let release = render(640, 2300);
        assert_eq!(differing_bytes(&release, &target), 0, "the release's 640");
        assert_eq!(differing_bytes(&tree, &target), 401, "the tree's 639");
        let differing_pixels = tree
            .chunks_exact(3)
            .zip(target.chunks_exact(3))
            .filter(|(mine, theirs)| mine != theirs)
            .count();
        assert_eq!(differing_pixels, 146);
    }

    #[test]
    fn the_trees_own_wait_would_hold_through_the_scroll_keyframes() {
        // The development snapshot's `:153-159` counterfactual: `musplus >= 0`
        // is first true at trace frame 2488 (order 47 row 0), which the port's
        // one-frame lead puts on tick 2068. Under that policy the part is
        // still waiting at every scroll keyframe and draws the held opening
        // there; the counts are the divergence A17 records, and the part's
        // own behaviour is the release's wait (`RELEASE_WAIT_TICKS`).
        let run = |tick: u32| -> (Phase, Vec<u8>) {
            let (mut rig, mut part) = started_with(scripted(tree_script(tick + 2)));
            part.tree_wait = true;
            run_to(&mut rig, &mut part, tick + 1);
            (part.phase, rig.rendered_rgb())
        };
        let (phase, held) = run(tick_of(500));
        assert!(matches!(phase, Phase::Wait { .. }));
        let mut counts = Vec::new();
        for index in [1100u32, 1700, 2300] {
            let tick = tick_of(index);
            let (phase, mine) = run(tick);
            assert!(matches!(phase, Phase::Wait { .. }), "boot {index}");
            assert_eq!(mine, held, "boot {index} is not the held opening");
            counts.push(differing_bytes(&mine, &reference("mntscrl", index, "rgb")));
        }
        assert_eq!(counts, [118_843, 118_924, 119_056]);
    }

    fn tick_of(index: u32) -> u32 {
        index - REFERENCE_FIRST_INDEX
    }
}
