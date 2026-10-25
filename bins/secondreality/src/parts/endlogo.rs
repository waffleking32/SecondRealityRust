//! ENDLOGO — the end picture: the FUTURE/CREW logo, white, faded in over a
//! held screen and faded out to black.
//!
//! `END/END.C` (130 lines) is the part itself, over `END/ASM.ASM` (`lineblit`,
//! `setpalarea`, and an `inittwk` that is dead code for this part) and
//! `END/READP.C` (the Origin RLE row reader, byte for byte the reader BEGLOGO
//! links). Its acts, in order:
//!
//! 1. `:51-60` — unlock the attribute controller and write AC 0x11 = 255.
//! 2. `:61` — one vertical blank.
//! 3. `:62-63` — point the DAC's write index at 0 and write 63 **765 times**:
//!    the three channels of entries 0-254, and **entry 255 is not written**.
//! 4. `:67-88` — CRTC 0x0c/0x0d = 0 (the display starts at the top of video
//!    memory), CRTC 9 with bit 7 and the low five bits cleared (a 400-line
//!    screen, one scanline a row), and AC 0x11 = 0 — the overscan colour.
//! 5. `:89` — one vertical blank; `:90-91` — the same 765 bytes of 63 again.
//! 6. `:92` — 32 vertical blanks.
//! 7. `:94-99` — `readp(palette,-1,pic)` and 400 `lineblit`ed rows.
//! 8. `:101-106` — 129 steps of a white→picture ramp, one per vertical blank.
//! 9. `:107-111` — the music gate: one blank a tick, until `dis_musplus()` passes
//!    `-16` or five thousand blanks have gone by. The port runs the source's own
//!    loop, against the run's player — see [`Endlogo::gate`] for where in the
//!    tick its check reads the music, which is where the reference's frames and
//!    the trace's words meet.
//! 10. `:112-117` — 64 steps of a picture→black ramp, one per vertical blank.
//!
//! Three things the part does **not** do, and each is a trap:
//!
//! - **It does not set its own video mode.** `:64-66`'s `mov ax,13h+80h` /
//!   `int 10h` / `inittwk()` are commented out, because the shell does it
//!   before `exe2021` (`MAIN/U2.ASM:906-908`, the same three lines `SECOND 2`
//!   runs at `:806-808`). `inittwk` is `END/ASM.ASM:7-50`: CRTC 9 = 0 — a
//!   400-line display — the CRTC 0x14/0x17 pair that takes the address counter
//!   to byte mode (`0xe317`), the sequencer's memory-mode and map-mask
//!   registers (`0x0604`, `0x0f02`), and 64 KB of video memory cleared to
//!   index 0. The port's shell already is that machine: its
//!   surface starts zeroed and its first part sets the view's geometry, and
//!   the byte-mode write has no analogue in a chunky model at all (there is no
//!   plane interleave to address). What is left for the part is the CRTC and
//!   AC state, which `END.C` writes again for itself at `:67-88`.
//! - **It does not clear video memory.** There is no `memset` in `END.C`: the
//!   screen at the reference's 430 is `inittwk`'s zeroed memory showing through
//!   a white DAC, which is why 430's indices are **all 0** and its pixels are
//!   all white.
//! - **It does not write DAC entry 255.** See [`white_dac_tick`].
//!
//! The port's tick numbering follows the source's vertical blanks one for one,
//! and ten of the ticks between the blit and the fade are frames the port does
//! not model — `docs/fidelity/APPROXIMATIONS.md` A7's second half, the same
//! class as A5's nine for BEGLOGO.
//!
//! **The gate is the part's only variable-length act.** Everything before it and
//! everything after it is a fixed tick count — the lead-in is the source's own
//! blanks and uploads, and the fade-out is 64 steps — so the part's tick map is
//! those consts plus whatever the music says the hold is. On the reference's
//! boot it is 567 ticks (`docs/fidelity/endlogo.md` §3's derivation, and §5 for
//! the cue that reproduces it).

use std::path::Path;

use sr_assets::load_path;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "endlogo";

/// The picture, read in its original format at runtime: `readp(palette,-1,pic)`
/// copies its palette out of the container and `:95-99` decodes 400 rows.
const PICTURE_PATH: &str = "assets/END/PIC.UH";

/// The picture's geometry, and the screen's: 320×400 Mode X, which `:67-80`
/// takes the display to (and `inittwk` had already taken it to).
const PICTURE_WIDTH: usize = 320;
const PICTURE_HEIGHT: usize = 400;

/// The overscan colour `:86-87` leaves in AC 0x11 — an entry *number*, not a
/// colour. The AC is written twice, 255 at `:55-57` and 0 here, so the last
/// write wins and the border shows DAC entry 0, whatever that entry holds: it
/// is white for the whole part until the fade-in carries it down to the
/// picture's own 0, which is black.
const OVERSCAN_ENTRY: u8 = 0;

/// The reference index a run that starts at this part draws its first frame at:
/// the demo's first graphics frame, which is also `shell::boot_anchor`'s entry
/// for `endlogo`. `PartCtx::frame` counts this part's ticks from there, so the
/// reference index any tick draws at is this plus `ctx.frame`.
const FIRST_INDEX: u32 = crate::shell::FIRST_GRAPHICS_FRAME;

/// `END.C:61`'s `dis_waitb()`: the first vertical blank, on which the part has
/// written nothing at all. The screen is `inittwk`'s — video memory cleared to
/// index 0, through the DAC the mode set left — which is what the port's first
/// tick shows.
const FIRST_WAIT_TICK: u32 = 0;

/// `:62-63`'s white: the tick after `:61`'s blank, and what [`white_dac_tick`]
/// is.
const WHITE_DAC_TICK: u32 = FIRST_WAIT_TICK + 1;

/// `:89`'s `dis_waitb()`, the second of the three the part spends before the
/// fade.
const SECOND_WAIT_TICK: u32 = WHITE_DAC_TICK + 1;

/// `:90-91`: the same 765 bytes of 63 written a second time, after the CRTC and
/// the AC are pointed at a 400-line display starting at video offset 0.
const SECOND_WHITE_TICK: u32 = SECOND_WAIT_TICK + 1;

/// `:92`'s `for(a=0;a<32;a++) dis_waitb()` — 32 vertical blanks of white.
const BLANKS_TICKS: u32 = 32;

/// The first of `:92`'s blanks: the tick after `:90-91`.
const BLANKS_FIRST_TICK: u32 = SECOND_WHITE_TICK + 1;

/// `:94-99`'s tick — `readp` and the 400 `lineblit`ed rows, which run on the
/// tick after the last blank and cost no blank of their own.
const BLIT_TICK: u32 = BLANKS_FIRST_TICK + BLANKS_TICKS;

/// The reference index the fade-in's step `c = 0` is displayed on.
///
/// Derived in `docs/fidelity/endlogo.md` §3 by matching the reference's own
/// frame at 500 bit for bit: the ramp's `c` is run over its whole range
/// `0…128` and exactly one reproduces that frame's 320×400 RGB, the picture
/// rendered through the widened DAC — `c = 49`, so the step *displayed* at 500
/// is the 50th and step 0 is 500 − 49 = **451**. A fit over
/// `reference-indices.md` §19.1's sweep means puts it "near 450"; the scan
/// separates 451 from 450 on every pixel they differ in.
///
/// The record checks it twice without being asked to (§19.1): the sweep's
/// first sample of the settled picture is **580**, and the derivation's last
/// fade-in step is displayed at 451 + 128 = **579**; and the record's samples
/// 580…1120 are one `sha256`, which is what a hold that starts at 580 means.
const FADE_IN_FIRST_INDEX: u32 = 451;

/// The part tick [`FADE_IN_FIRST_INDEX`] is displayed on.
///
/// The source accounts for 37 of these 47 ticks: 34 vertical blanks (`:61`,
/// `:89` and `:92`'s 32) and three ticks the code between them runs on
/// (`:62-63` with `:67-88`, `:90-91`, and `:94-99`). **The other ten are
/// frames the port does not model**, and their cause is unidentified —
/// `docs/fidelity/APPROXIMATIONS.md` A7, the same class as A5's nine between
/// BEGLOGO's `:24` and its fade, and counted the same way: the fade's own
/// `:104` blank is not a tick of its own but part of the iteration that
/// displays step 0 at tick 47, exactly as `BEG/BEG.C:84`'s is of the one that
/// displays BEGLOGO's step 0 at its tick 42. The port draws `inittwk`'s screen
/// (index 0) and then the white field through the ten.
const FADE_IN_FIRST_TICK: u32 = FADE_IN_FIRST_INDEX - FIRST_INDEX;

/// `:101`'s `for(c=0;c<=128;c++)`: 129 fade steps, one per tick from
/// [`FADE_IN_FIRST_TICK`]. `c = 128` is the picture's own palette exactly.
const FADE_IN_STEPS: u32 = 129;

/// `:112`'s `for(c=63;c>=0;c--)`: 64 fade steps, one per tick. Note that the
/// loop counts **down**, so the step displayed on the `step`-th tick of the
/// fade-out is `63 - step` ([`Endlogo::fade_out`]).
const FADE_OUT_STEPS: u32 = 64;

/// `:110`'s `dis_musplus()>-16`: the row offset the gate waits to pass.
const GATE_THRESHOLD: i32 = -16;

/// `:107`'s `a<5000`: the loop's own bound, in iterations — which is also its
/// only bound on the hold, and stays (the source's cap, not a port constant).
const GATE_LIMIT: u32 = 5000;

/// How many ticks the port holds the gate **past its own break**, so that the
/// hold ends where the reference's *frames* put it. One, and it is the one frame
/// the phase's two measurements are apart.
///
/// The break itself is the source's: the check reads the words the tick before
/// published them ([`Endlogo::gate`]), so the tick it breaks on is the frame the
/// source's `break` is drawn on, and `:112`'s fade-out would start the tick
/// after that — which is what this constant delays.
///
/// **Why.** The port's player is `reference/music/second5`'s: its published row
/// 49 begins at the run's index **1144**, and the gate's break therefore falls
/// on index **1145**. The reference's own frames put the fade-out's first step
/// at **1147** (`docs/fidelity/endlogo.md` §3, derived bit-exactly from the
/// frames at 500 and 1180), i.e. the break at **1146**. The frame between them is
/// the session difference this task measured and named: `second5` publishes row
/// 49 from its frame 1145, so that session's music crossed one frame earlier
/// than the session the reference's frames were captured in — D17's own finding,
/// that a frame counter names one picture in a session and a picture one frame
/// away in the next. **D12 makes the reference's frames the record for every
/// comparison**, so the port sits where they do; the cost is that the port
/// disagrees with `second5`'s own session by one frame at this crossing, which
/// is inside the ±3 frames the two filed sessions of one recipe measure on each
/// other (`APPROXIMATIONS.md` A12) and inside the 2 frames the port's own
/// position already sits from that trace.
///
/// It applies to the cue's break and **not** to `GATE_LIMIT`: the loop's own
/// bound is the source's answer when the cue never comes, and no reference frame
/// measures it.
const GATE_ALIGNMENT_TICKS: u32 = 1;

/// What a run with no player answers the gate, unreachable through `init`.
///
/// `muscode_6` loads `%dx` with `-32` as its "no plus pending" sentinel
/// (`DIS/DISINT.ASM:252`), which is also what `sr_engine::music::NoMusic`
/// reports for a player that has nothing pending; the gate's threshold is above
/// it, so a run that somehow reached here without music would hold for the
/// loop's own five thousand ticks and then fade out, as the source's loop does
/// when the cue never comes.
const NO_PLAYER: i32 = -32;

/// `:105`/`:116`'s `setpalarea(pal2,0,255)` — the third argument, which the
/// routine treats as a *count* of entries but which the buggy length makes two
/// bytes of each ([`setpalarea`]).
const SETPALAREA_ENTRIES: u32 = 255;

/// The picture and the palette it indexes, both as `readp` leaves them.
pub struct Endlogo {
    /// Where the picture is read from. [`PICTURE_PATH`] in a run — the binary
    /// is started from the repository root, which is where `assets/` is — and
    /// the same file by the path that works from this crate's directory in a
    /// unit test.
    picture_path: &'static str,
    /// The decoded picture, row-major at [`PICTURE_WIDTH`] — the shape
    /// `readp(rowbuf,y,pic)` decodes one row of. Empty until `init`.
    picture: Vec<u8>,
    /// The container's palette, six bits a channel: the source of both fades
    /// and, at the end of the fade-in, the screen's own colours.
    /// `END/END.C` calls it `palette[]`; it is not the live DAC.
    picture_palette: Palette,
    /// The last `dis_musplus()` reading this part took, which is the one the
    /// *next* tick's gate check tests — see [`Endlogo::gate`] for why the check
    /// is a tick behind the reading.
    plus: i32,
    /// Where the part is in `main()`. The lead-in's ticks are the source's own
    /// and fixed; the gate's length is the music's, so the loops after it are a
    /// state and not a tick number.
    phase: Phase,
}

/// `main()`'s three loops, as states.
///
/// The lead-in is a tick number because every tick of it is fixed by the
/// source's own code; the gate and the fade-out are counts because the gate's
/// end is the music's business ([`Endlogo::gate`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `:61-106`: the blanks, the two white writes, the blit and the 129-step
    /// fade-in, driven by `ctx.frame`.
    LeadIn,
    /// `:107-111`: the music gate. `count` is the source's `a` — the iteration
    /// this tick is, and so the tick the hold is `count + 1` long on. `held` is
    /// how many ticks the port has held past the break's own frame, the break
    /// not counted ([`GATE_ALIGNMENT_TICKS`]); it is 0 while the gate is open.
    Gate { count: u32, held: u32 },
    /// `:112-117`: the fade-out. `step` is 0 on the tick the loop's `c = 63` is
    /// displayed on and 63 on its last, so the displayed `c` is
    /// `FADE_OUT_STEPS - 1 - step`.
    FadeOut { step: u32 },
    /// `:118-130`: `main()` has returned and the loader holds the black screen.
    Done,
}

impl Endlogo {
    /// A part with no picture: `init` is what reads one.
    pub fn new() -> Endlogo {
        Endlogo {
            picture_path: PICTURE_PATH,
            picture: Vec::new(),
            picture_palette: Palette::new(),
            plus: NO_PLAYER,
            phase: Phase::LeadIn,
        }
    }

    /// [`new`](Endlogo::new) reading the picture by `path`.
    ///
    /// A unit test's working directory is this crate's, not the repository
    /// root, so the part's own relative path does not resolve there; the
    /// integration test in `tests/endlogo.rs` runs the real binary from the
    /// repository root, where it does.
    #[cfg(test)]
    fn at(path: &'static str) -> Endlogo {
        Endlogo { picture_path: path, ..Endlogo::new() }
    }
}

impl Part for Endlogo {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `END.C:51-99` minus everything the part times: the attribute
    /// controller's PAS write, the CRTC registers `:67-80` and the AC's
    /// overscan `:86-87`, and the picture read.
    ///
    /// The CRTC and AC writes are `init`'s and not a tick's because they cost
    /// no frame — `:61`'s blank is what separates them from `main()`'s start,
    /// and the port pays that blank as [`FIRST_WAIT_TICK`] — and because the
    /// port's view is a description of the display rather than a register
    /// write, which is where BEGLOGO's CRTC block lives too (`src/parts/
    /// beglogo.rs`, `docs/fidelity/beglogo.md` §4).
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:107-111` waits on the music player's position and has nothing else to go on:
        // a run with no player has no cue this part could ever be handed, and holding
        // five thousand ticks against a silence that cannot break is not a fallback — it
        // is a part that looks like it works. `PartCtx::music()` is `Some` for every run
        // the shell can build that reaches this part (`shell::music_for_part`), and the
        // refusal is the port's way of saying so where a hand-built run does not
        // (`crates/sr-engine/src/part.rs:154-165`).
        if ctx.music().is_none() {
            return Err(PartError {
                part: NAME,
                message: "this part waits on the music player's position (`END/END.C:107-111`) \
                          and the run has no music to wait on"
                    .to_string(),
            });
        }

        // `:51-60`'s attribute-controller unlock ends in `outp(0x3c0,0x20)` —
        // the index register set to 0 with bit 5, the palette address source —
        // and PAS = 1 is the state in which the DAC's palette is displayed at
        // all. The port's engine always displays it, so there is nothing here
        // to set. BEGLOGO folds the same writes the same way.
        //
        // `:67-74`: CRTC 0x0c/0x0d = 0. That register pair is one 16-bit
        // display start, which is why `set_origin(0)` zeroes `start_addr` **and**
        // `x_offset` rather than leaving the offset alone. The registers
        // `END.C` does not write — AC 0x13's pan, CRTC 0x18's line compare,
        // CRTC 0x13's line offset — are left as the run left them: no split.
        //
        // **The line offset is the one the original's picture is drawn at.**
        // Every part the demo runs before this one (U2E, in blocks 1, 3 and 4)
        // leaves the 320-pixel tweak's CRTC 0x13 — 80 bytes a row — which is the
        // stride `:95-99`'s `lineblit(vram+(unsigned)y*80U,rowbuf)` writes, so
        // the original reads its picture row by row. The port draws through
        // `Surface::set`, at the surface's own 640-pixel rows, read by the unset
        // offset (Part B's first convention); U2E's `Some(320)` kept here would
        // read every second scanline from the picture's right margin, which is
        // what block 1's ENDLOGO showed until the full run's comparison found it
        // (`reference-indices.md` §26.8).
        ctx.view.y_line_offset = None;
        ctx.view.set_origin(0);
        // `:75-80`: CRTC 9 with bit 7 (the double-scan bit) and the low five
        // bits (the maximum scan line) cleared — a 400-line screen, one
        // scanline a row — and `:86-87`'s AC 0x11 = 0, the overscan colour.
        ctx.view.height = PICTURE_HEIGHT as u16;
        ctx.view.double_scan = false;
        ctx.view.border = Some(OVERSCAN_ENTRY);

        // `:94`'s `readp(palette,-1,pic)` copies the palette out of the
        // container and `:95-99` decodes 400 rows through it. The port decodes
        // the whole container in one call, which is the same bytes — and the
        // derivation test below is where that is measured: the decode is
        // byte-identical to the reference's held frame at 900, 0 of 128,000
        // pixels apart (`docs/fidelity/endlogo.md` §8).
        let image = load_path(Path::new(self.picture_path)).map_err(|error| PartError {
            part: NAME,
            message: format!("{}: {error}", self.picture_path),
        })?;
        let (width, height) = (image.width() as usize, image.height() as usize);
        if (width, height) != (PICTURE_WIDTH, PICTURE_HEIGHT) {
            return Err(PartError {
                part: NAME,
                message: format!(
                    "{} is {width}x{height}; this part's screen is \
                     {PICTURE_WIDTH}x{PICTURE_HEIGHT}",
                    self.picture_path
                ),
            });
        }
        self.picture = image.pixels().to_vec();
        self.picture_palette = image.palette;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `:110`'s reading, taken once a tick — the whole of what this part wants from
        // the player, and taken on every tick rather than only inside the gate because
        // the tick the gate's first check runs on needs the reading the tick before it
        // took (see [`Endlogo::gate`]).
        let reading = match ctx.music() {
            Some(music) => music.musplus(),
            // Unreachable through `init`, which refuses a run with no player; see
            // [`NO_PLAYER`].
            None => NO_PLAYER,
        };
        // The reading this tick's check tests, and the reading the next tick's will:
        // the source's check stands at the frame boundary, before the copper polls the
        // player for that frame, so it reads the words the *previous* tick published.
        let previous = self.plus;
        self.plus = reading;

        match self.phase {
            Phase::LeadIn => self.lead_in(ctx),
            Phase::Gate { count, held } => self.gate(previous, count, held),
            Phase::FadeOut { step } => self.fade_out(ctx, step),
            // `:118-129`'s epilogue is commented out and `:130`'s `}` ends `main()`: the
            // loader holds the black screen the fade left, and everything after this
            // index is the next part's.
            Phase::Done => PartStatus::Finished,
        }
    }
}

impl Endlogo {
    /// `:61-106`: the blanks, the two white writes, the blit, the ten unmodelled ticks
    /// and the 129-step fade-in.
    ///
    /// Every tick of it is fixed by the source's own code — which is why this half of
    /// the part is tick numbers and the loops after it are not — and the last fade-in
    /// step is the tick the gate's first iteration is reached on.
    fn lead_in(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let tick = ctx.frame;
        match tick {
            // `:61` — the vertical blank before the DAC is written. The screen
            // is `inittwk`'s index 0 through the DAC the shell installed.
            FIRST_WAIT_TICK => PartStatus::Continuing,
            // `:62-63` — the white.
            WHITE_DAC_TICK => white_dac_tick(ctx),
            // `:89` — the blank between the two white writes.
            SECOND_WAIT_TICK => PartStatus::Continuing,
            // `:90-91` — the white again, after the display is pointed at a
            // 400-line screen.
            SECOND_WHITE_TICK => white_dac_tick(ctx),
            // `:92`'s 32 vertical blanks.
            tick if tick < BLIT_TICK => PartStatus::Continuing,
            // `:94-99` — the picture into video memory.
            BLIT_TICK => {
                self.blit(ctx);
                PartStatus::Continuing
            }
            // The ten ticks between `:99` and the fade's first upload: frames
            // the port does not model (A7's second half). The screen is white through
            // them because the DAC is, not because the picture is invisible.
            tick if tick < FADE_IN_FIRST_TICK => PartStatus::Continuing,
            tick => {
                // `:103-105` — one step of the white→picture ramp per vertical blank.
                let step = tick - FADE_IN_FIRST_TICK;
                let ramp = fade_in_step(&self.picture_palette, step);
                setpalarea(ctx.palette, &ramp, 0, SETPALAREA_ENTRIES);
                if step + 1 == FADE_IN_STEPS {
                    // `:101`'s loop is done: the next tick is `:107`'s first iteration,
                    // and `dis_musplus()`'s answer from the tick just gone is already
                    // in `self.plus` for its check.
                    self.phase = Phase::Gate { count: 0, held: 0 };
                }
                PartStatus::Continuing
            }
        }
    }

    /// `:107-111`: `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb(); if(dis_musplus()>-16)
    /// break; }` — one iteration a tick, and the loop's body has no DAC write of its own,
    /// so the held frame is the fade-in's last step and this tick is the blank the
    /// iteration waits on. `count` is the source's `a`.
    ///
    /// **The check is made on the words the *previous* tick published.** The source's check
    /// is the instruction after `dis_waitb()` returns, and `dis_waitb()` returns on the
    /// copper's own frame boundary: `waitb_1` (`DIS/DISINT.ASM:161-181`) spins on
    /// `cs:copperframecount`, which `copper_end` (`MAIN/COPPER.ASM:283-320`) increments
    /// (`:320`) right after its wait for the vertical retrace (`:305-311`). The music's
    /// poll is not there: it is `call _zpollme` (`MAIN/COPPER.ASM:466`) inside
    /// `copper_doit`, which the copper's list runs at 20 of the frame's 100 units
    /// (`copperlst0`, `:41-43`) — a fifth of a frame *after* the boundary the check stands
    /// on. So the words that check reads are the ones the **previous** frame's poll
    /// published.
    ///
    /// The port's engine polls between copper slot 0 and the first band
    /// (`crates/sr-engine/src/engine.rs:206-219`), so a part's `musplus()` at tick `T`
    /// answers with the words tick `T`'s poll published, which is what the source's check at
    /// the boundary into the *next* frame reads. That is where this part keeps the reading:
    /// it tests `previous`, the answer [`Endlogo::frame`] took on the tick before, and the
    /// tick it fires on is the tick the source's own break happens on.
    ///
    /// **And the break is held one tick longer than the source's loop**, because the
    /// reference's frames — the record for every comparison (D12) — put the fade-out one
    /// frame later than the port's own words do: [`GATE_ALIGNMENT_TICKS`] carries that
    /// measurement and its cost. `the_gate_opens_on_the_tick_the_reference_frames_derived`
    /// is the check, and `docs/fidelity/endlogo.md` §5 the account.
    fn gate(&mut self, previous: i32, count: u32, held: u32) -> PartStatus {
        if previous > GATE_THRESHOLD || held > 0 {
            // The cue has crossed — on this tick, or on one of the ticks the port holds
            // past it. `held` counts the ticks held since the break, the break's own frame
            // not among them, so the fade-out's phase is entered on the tick `held` reaches
            // [`GATE_ALIGNMENT_TICKS`] — and, because entering it is not the same as
            // displaying it, its first step is the tick after that. With the alignment at
            // one: the break's frame, one held frame, then `:112`'s `c = 63`.
            if held >= GATE_ALIGNMENT_TICKS {
                self.phase = Phase::FadeOut { step: 0 };
            } else {
                self.phase = Phase::Gate { count, held: held + 1 };
            }
        } else if count + 1 == GATE_LIMIT {
            // `a` has reached five thousand and the loop is left without a break. The
            // loop's own bound is the source's and no reference frame measures it, so the
            // alignment is not applied to it.
            self.phase = Phase::FadeOut { step: 0 };
        } else {
            self.phase = Phase::Gate { count: count + 1, held: 0 };
        }
        PartStatus::Continuing
    }

    /// `:112-117`: `for(c=63;c>=0;c--) { pal2 = (palette*c)/64; dis_waitb();
    /// setpalarea(pal2,0,255); }` — one step a tick, counting down from the tick the
    /// loop's `c = 63` is displayed on.
    fn fade_out(&mut self, ctx: &mut PartCtx, step: u32) -> PartStatus {
        // `:114-116` — one step of the picture→black ramp per vertical blank.
        let ramp = fade_out_step(&self.picture_palette, FADE_OUT_STEPS - 1 - step);
        setpalarea(ctx.palette, &ramp, 0, SETPALAREA_ENTRIES);
        self.phase = if step + 1 < FADE_OUT_STEPS {
            Phase::FadeOut { step: step + 1 }
        } else {
            Phase::Done
        };
        PartStatus::Continuing
    }
}

/// `END.C:62-63` and `:90-91`: `outp(0x3c8,0)` and then `for(a=0;a<768-3;a++)
/// outp(0x3c9,63)`.
///
/// The loop runs 765 times and each iteration is **one byte**: three of them
/// make an entry, so this fills the three channels of DAC entries 0-254 and
/// **leaves entry 255 unwritten**. That is the opposite of BEGLOGO, whose
/// `BEG/BEG.C:25-34` writes 255 triples and then a black entry 255
/// (`src/parts/beglogo.rs`): the two screens are the same white — the record's
/// `85789170…` is both part's opening frame — and the two DACs differ in
/// exactly the one entry neither of them displays.
///
/// The port writes the entries in place rather than replacing the palette,
/// which is how entry 255 keeps whatever the part before this one left. What
/// it holds on the reference's boot is the loader's, and no reference frame
/// shows it: the screen at 430 is index 0, and the picture indexes 0-71.
fn white_dac_tick(ctx: &mut PartCtx) -> PartStatus {
    for entry in 0..=254u8 {
        ctx.set_palette_entry(entry, [63, 63, 63]);
    }
    PartStatus::Continuing
}

impl Endlogo {
    /// `END.C:95-99`: `readp(rowbuf,y,pic)` and `lineblit(vram+y*80,rowbuf)`,
    /// one row per video row.
    ///
    /// `lineblit` (`END/ASM.ASM:53-82`, the four-plane loop at `:63-76`) is a
    /// *plane* routine: for each of the four planes it writes 80 bytes to
    /// `es:[di+zzz]`, which is 320 pixels of one video row, and `vram + y*80`
    /// is that row's plane offset. The port writes the same 320 pixels through
    /// [`Surface::set`] at the surface's own row stride, with `y_line_offset`
    /// left unset (Part B's first convention): the plane interleave is a
    /// hardware detail the chunky model folds away, and the picture the beam
    /// reads back is the same one.
    ///
    /// [`Surface::set`]: sr_engine::Surface::set
    fn blit(&self, ctx: &mut PartCtx) {
        for (y, row) in self.picture.chunks_exact(PICTURE_WIDTH).enumerate() {
            for (x, pixel) in row.iter().enumerate() {
                ctx.surface.set(x, y, *pixel);
            }
        }
    }
}

/// `END.C:103`: `pal2[a] = ((128-c)*63 + palette[a]*c)/128`, integer division,
/// over six-bit entries — a straight line from white (63) at `c = 0` to the
/// picture's own palette at `c = 128`.
///
/// The original fills `a = 0..765` (`768-3`, so entries 0-254) and this
/// computes all 256; the difference cannot be read, because the upload stops
/// inside entry 169 ([`setpalarea`], below).
fn fade_in_step(picture: &Palette, c: u32) -> Palette {
    let mut ramp = Palette::new();
    for index in 0..=255u8 {
        let entry = picture.entry(index);
        let mut rgb = [0u8; 3];
        for (channel, value) in rgb.iter_mut().enumerate() {
            *value = (((128 - c) * 63 + entry[channel] as u32 * c) / 128) as u8;
        }
        ramp.set_entry(index, rgb);
    }
    ramp
}

/// `END.C:114`: `pal2[a] = (palette[a]*c)/64`, integer division, over six-bit
/// entries — a straight line from the picture's palette at `c = 64` down to
/// black at `c = 0`, of which the loop displays `c = 63` … `c = 0`.
///
/// It is **not** the fade-in reversed: that one comes *from* white and this one
/// goes *to* black, so `c = 63` is the picture one step down rather than
/// anywhere the fade-in passed through.
fn fade_out_step(picture: &Palette, c: u32) -> Palette {
    let mut ramp = Palette::new();
    for index in 0..=255u8 {
        let entry = picture.entry(index);
        let mut rgb = [0u8; 3];
        for (channel, value) in rgb.iter_mut().enumerate() {
            *value = (entry[channel] as u32 * c / 64) as u8;
        }
        ramp.set_entry(index, rgb);
    }
    ramp
}

/// `END.C:105`/`:116`'s `setpalarea(pal2,0,255)`, which is `END/ASM.ASM:93-115`
/// — **with the byte-count bug the original ships, which the port preserves**
/// (`docs/fidelity/bugs.md` §4, where this routine is the entry).
///
/// `:106-108` loads `count` into `cx`, `shl cx,1` — `2*count` — and `add cx,ax`
/// (`start`), then `REPOUTSB` moves exactly that many bytes from the DAC's
/// write index, which `:104-105` points at `start` (`0x3c8` then the index
/// byte). So the length is **`2*count + start` bytes, not `3*count`**, and
/// `setpalarea(ramp, 0, 255)` uploads **510** bytes: entries 0-169 whole, and
/// nothing at all of entries 170-255.
///
/// **510 and not the 508 of BEGLOGO's copy, and the difference is the call.**
/// `BEG/BEG.C:84` passes 254 and `END.C:105` passes 255 — the two source files
/// name different spans, and the same routine doubles each. This part's
/// `:90-91` white reaches entries 0-254 and both fades reach 0-169, so entries
/// 170-254 stay white for the whole part either way; the number is written
/// down because it is the source's, not because a keyframe can see it.
///
/// This is `BEG/ASM.ASM:93-115` again — the tree carries one copy of the
/// routine per part directory, and the port carries one per part module.
fn setpalarea(dac: &mut Palette, ramp: &Palette, start: u32, count: u32) {
    let bytes = 2 * count + start;
    for written in 0..bytes {
        let at = 3 * start + written;
        // The DAC's write index is eight bits and wraps, which no call here
        // reaches: this part's three calls write 510 bytes from entry 0.
        let entry = (at / 3 % 256) as u8;
        let channel = (at % 3) as usize;
        let mut rgb = dac.entry(entry);
        rgb[channel] = ramp.entry(entry)[channel];
        dac.set_entry(entry, rgb);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::{muscode_from, musplus_from, Music, MusicPosition};
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::surface::Surface;
    use sr_engine::view::View;

    use crate::shell::{SequenceStatus, SURFACE_HEIGHT, SURFACE_WIDTH, VIEW_HEIGHT, VIEW_WIDTH};

    /// An index no path through the part writes, used as a marker for "the part
    /// left the shell's entry alone".
    const UNWRITTEN: u8 = 255;

    // --- the reference's own map, kept as the check --------------------------
    //
    // These six are **not the part's** any more: the gate's length is the music's, and
    // the only thing that says where the hold ends is `:107-111`'s own loop
    // (Task 8). They are what Phase 2 derived from the reference's *frames*, kept so
    // that the cue's own answer can be compared with it — the check the brief calls the
    // phase's proof. `docs/fidelity/endlogo.md` §3 is the derivation and §5 the
    // comparison.

    /// The reference index the fade-out's step `c = 63` is displayed on.
    ///
    /// Derived in `docs/fidelity/endlogo.md` §3 by matching the reference's frame at
    /// 1180 bit for bit: one `c` in `63…0` reproduces it — `c = 30` — and the fade-out
    /// counts *down*, so the step displayed at 1180 is the 34th and step 63 is displayed
    /// at 1180 − 33 = **1147**. The picture is byte-identical from 580 to 1120 (§19.1)
    /// and its last identical frame is 1146, so the hold is real; and the source's
    /// 64-step fade-out runs from 1147 to 1210, inside the window **(1200, 1240]** in
    /// which `SECOND 5` leaves ENDLOGO's screen for CRED's mode 13h one (§20).
    const FADE_OUT_FIRST_INDEX: u32 = 1147;

    /// The first index of the settled picture: the tick after the fade-in's last step.
    const HOLD_FIRST_INDEX: u32 = FADE_IN_FIRST_INDEX + FADE_IN_STEPS;

    /// How many ticks the reference's frames show the settled picture held for — the
    /// gate's length on the reference's boot, and the number the cue has to reproduce.
    const HOLD_TICKS: u32 = FADE_OUT_FIRST_INDEX - HOLD_FIRST_INDEX;

    /// The part tick [`FADE_OUT_FIRST_INDEX`] is displayed on.
    const FADE_OUT_FIRST_TICK: u32 = FADE_OUT_FIRST_INDEX - FIRST_INDEX;

    /// The reference index the part's last frame is displayed on: the fade-out's step
    /// `c = 0`, after which `:118-129`'s commented-out epilogue and `:130`'s `}` end
    /// `main()`. 1210 is inside the window **(1200, 1240]** in which `SECOND 5` leaves
    /// ENDLOGO's screen for CRED's (`reference-indices.md` §20).
    const PART_LAST_INDEX: u32 = FADE_OUT_FIRST_INDEX + FADE_OUT_STEPS - 1;

    /// [`PART_LAST_INDEX`] in the part's own tick numbering.
    const PART_LAST_TICK: u32 = PART_LAST_INDEX - FIRST_INDEX;

    /// The player a unit test hands the part: the engine's [`Music`] seam, scripted.
    ///
    /// `init` refuses a run with no player, so every test that starts this part has to
    /// hand it one; this stands in for `src/audio.rs`'s adapter and answers
    /// `musplus()` from a script of `(zplus, row)` pairs, **one per tick**, indexed by
    /// `ctx.frame`. Every other method is the null reading, because the gate is the only
    /// thing here that reads the music (`DIS/DIS.H:37-59` is the whole of a part's
    /// access, and this part uses one call of it).
    struct Player {
        /// `(zplus, row)` a tick, indexed by `ctx.frame`; empty means the object's own
        /// "no plus pending" sentinel at every tick.
        script: Vec<(i32, i32)>,
        /// The tick the rig is on — the rig sets it before each `ctx`.
        at: usize,
    }

    impl Player {
        /// A player with no plus pending: `musplus()` is `-32` at every tick.
        fn sentinel() -> Player {
            Player { script: Vec::new(), at: 0 }
        }

        /// A player that answers `(zplus, row)` from `script`, one entry a tick.
        fn scripted(script: Vec<(i32, i32)>) -> Player {
            Player { script, at: 0 }
        }

        /// What the port's player publishes at the reference index `index` — this
        /// script's answer for that tick — without moving the rig's own tick.
        fn reading_at(&self, index: u32) -> i32 {
            match self.script.get((index - FIRST_INDEX) as usize) {
                Some(&(zplus, row)) => musplus_from(zplus, row),
                None => musplus_from(0, 0),
            }
        }

        /// **`reference/music/second5`'s own words, as the port's player publishes them.**
        ///
        /// The trace's columns are the published words (`_np_row`, `_np_zplus`) and it is
        /// indexed by the *reference's* frame number — and **its sample for a frame is
        /// taken at that frame's start**, before the copper polls the player for it: the
        /// emulator's counter is incremented when a frame is published, which is the
        /// instant the guest's own code for that frame runs (`reference-indices.md` D17's
        /// "the emulator publishes a frame's pixels and counts it immediately
        /// afterwards"), so `second5`'s frame `N` holds the words of the poll during frame
        /// `N - 1`. The port's tick `N` publishes the words of the poll during frame `N` —
        /// the same player, one tick of the *tick* clock later, which is why the port's
        /// published row 49 begins at index 1144 where the trace's begins at frame 1145.
        ///
        /// So the script is the trace's words one reference index on: entry `t` is the
        /// trace's frame `FIRST_INDEX + t + 1`, and a tick the trace cannot answer for —
        /// past its last sample — gets the sentinel. The trace's three gaps (147, 409,
        /// 1220) are filled by carrying the previous sample forward, which is what the
        /// words do across a frame no sample was taken on: 408 and 410 are `(0, 0)` with
        /// `zplus` 0 in this trace.
        fn trace_second5() -> Player {
            let mut script: Vec<(i32, i32)> = Vec::new();
            for (frame, _order, zplus, row) in trace_second5_samples() {
                // Before the part's first tick there is no tick of this part's to answer
                // for; the trace's own window starts at 148, well before 404.
                if frame < FIRST_INDEX + 1 {
                    continue;
                }
                let at = (frame - FIRST_INDEX - 1) as usize;
                // The gaps: carry the previous sample forward.
                while script.len() < at {
                    script.push(*script.last().unwrap_or(&(0, 0)));
                }
                script.push((zplus, row));
            }
            Player::scripted(script)
        }
    }

    /// The words `reference/music/second5` holds, a frame each, as
    /// `(reference frame, order, `zplus`, row)`.
    ///
    /// The trace file is read as text, field by field, the way `src/shell.rs`'s own trace
    /// reader does: it is a committed record under `reference/`, and reading it is what
    /// lets a test quote the trace's own numbers instead of restating them.
    fn trace_second5_samples() -> Vec<(u32, u8, i32, i32)> {
        let path = "../../reference/music/second5/trace.csv";
        let text = std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        text.lines()
            .skip(1)
            .map(|line| {
                let fields: Vec<&str> = line.split(',').collect();
                let number = |index: usize| -> i64 {
                    fields
                        .get(index)
                        .unwrap_or_else(|| panic!("{line}: no field {index}"))
                        .trim()
                        .parse()
                        .unwrap_or_else(|error| panic!("{line}: field {index}: {error}"))
                };
                (number(0) as u32, number(1) as u8, number(8) as i32, number(2) as i32)
            })
            .collect()
    }

    /// What the trace holds at a reference frame, as `("order:row", musplus)`.
    fn trace_at(frame: u32) -> (String, i32) {
        let (_, order, zplus, row) = *trace_second5_samples()
            .iter()
            .find(|(at, _, _, _)| *at == frame)
            .unwrap_or_else(|| panic!("the trace holds no frame {frame}"));
        (format!("{order}:{row}"), musplus_from(zplus, row))
    }

    impl Music for Player {
        fn poll(&mut self) {}

        fn position(&self) -> MusicPosition {
            MusicPosition::default()
        }

        fn musplus(&mut self) -> i32 {
            match self.script.get(self.at) {
                Some(&(zplus, row)) => musplus_from(zplus, row),
                None => musplus_from(0, 0),
            }
        }

        fn musrow(&mut self) -> i32 {
            0
        }

        fn muscode(&mut self, wanted: i32) -> i32 {
            muscode_from(MusicPosition::default(), wanted)
        }

        fn sync(&mut self) -> i32 {
            unreachable!("`END.C` never syncs: the part's one music call is `dis_musplus()`")
        }

        fn getmframe(&mut self) -> i32 {
            0
        }

        fn setmframe(&mut self, _frame: i32) {}
    }

    /// The engine's own pieces held together, so a `PartCtx` can be built for
    /// one call at a time — the same disjoint borrows `Engine::tick` makes.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
        player: Player,
    }

    impl Rig {
        /// The screen and the DAC the shell hands a run's first part: a 320×200
        /// surface at the demo's size, and `hardware_default_palette`'s entry 0
        /// — `(0,0,0)` — which is what makes the reference's 404 black before
        /// the part has written a single DAC entry of its own.
        fn new() -> Rig {
            let mut palette = Palette::new();
            // The shell's table: entry 0 black, and an entry the part never
            // writes holding a marker, so "left alone" is distinguishable from
            // "written black".
            palette.set_entry(UNWRITTEN, [1, 2, 3]);
            Rig {
                surface: Surface::new(SURFACE_WIDTH, SURFACE_HEIGHT),
                view: View::new(VIEW_WIDTH, VIEW_HEIGHT),
                palette,
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
                player: Player::sentinel(),
            }
        }

        /// [`new`](Rig::new) with `player` standing in for the run's.
        fn with_player(player: Player) -> Rig {
            Rig { player, ..Rig::new() }
        }

        /// One tick's context, with the player at `frame`.
        fn ctx(&mut self, frame: u32) -> PartCtx<'_> {
            self.player.at = frame as usize;
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(&mut self.player),
                frame,
                0,
                1,
                true,
                false,
                false,
            )
        }

        /// One tick's context for a run with **no** player.
        fn ctx_without_music(&mut self, frame: u32) -> PartCtx<'_> {
            PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                None,
                frame,
                0,
                1,
                true,
                false,
                false,
            )
        }
    }

    /// A part whose `init` has run, and the rig it ran against.
    ///
    /// The picture is the curated `END/PIC.UH`, by the path that resolves from
    /// this crate's own directory — a unit test's working directory is not the
    /// repository root. `tests/endlogo.rs` runs the real binary, and so the
    /// part's own `assets/END/PIC.UH`, from there. The player is the sentinel
    /// one: a test that needs the gate to open builds its own rig.
    fn started() -> (Rig, Endlogo) {
        started_with(Rig::new())
    }

    /// [`started`] against a rig of the caller's own.
    fn started_with(mut rig: Rig) -> (Rig, Endlogo) {
        let mut part = Endlogo::at("../../assets/END/PIC.UH");
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    #[test]
    fn the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left() {
        // Mid-chain the view arrives with the predecessor's line offset: U2E (`u2e.rs`, `Some(VIEW_WIDTH)`) leaves it
        // at 320 pixels a row. In the original that is the 320-pixel tweak's CRTC 0x13,
        // the stride this part's own `lineblit(vram+y*80U, ...)` draws at, so the
        // picture is read row by row. On the port's 640-wide surface a kept `Some(320)`
        // would read every second scanline from the picture's right margin — the full
        // run's measurement (`reference-indices.md` §26.8) — so the part reads its own
        // rows, the unset offset.
        let mut rig = Rig::new();
        rig.view.y_line_offset = Some(320);
        let mut part = Endlogo::at("../../assets/END/PIC.UH");
        part.init(&mut rig.ctx(0)).unwrap();
        assert_eq!(rig.view.y_line_offset, None);
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Endlogo, ticks: u32) {
        for at in 0..ticks {
            part.frame(&mut rig.ctx(at));
        }
    }

    /// The DAC's entries 0-169, which is every entry the picture or a fade writes.
    fn dac_entries(rig: &Rig) -> Vec<[u8; 3]> {
        (0..=169u8).map(|index| rig.palette.entry(index)).collect()
    }

    /// The DAC the part's settled picture leaves: the fade-in's last step.
    fn settled(part: &Endlogo) -> Vec<[u8; 3]> {
        (0..=169u8).map(|index| part.picture_palette.entry(index)).collect()
    }

    #[test]
    fn the_white_leaves_entry_255_unwritten() {
        // `END.C:62-63` points the DAC's write index at 0 and writes 63 765
        // times — 765 *bytes*, which is the three channels of entries 0-254.
        // Entry 255 is not written: unlike `BEG/BEG.C:25-34`, no black goes
        // into it, and it keeps whatever the part before this one left. The
        // two screens are still the same white, which is why the record's
        // `85789170…` is both parts' opening frame.
        let (mut rig, mut part) = started();
        let before = rig.palette.entry(UNWRITTEN);
        assert_ne!(before, [63, 63, 63], "the marker entry is not already white");

        // `:62-63`, on the tick after the first blank.
        run_ticks(&mut rig, &mut part, WHITE_DAC_TICK + 1);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "`:63`'s 765 bytes stop short");

        // `:90-91` writes the same 765 bytes a second time, after the CRTC and
        // the AC are written, and still stops short.
        run_ticks(&mut rig, &mut part, SECOND_WHITE_TICK + 1 - WHITE_DAC_TICK - 1);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "`:91`'s 765 bytes stop short too");

        // And the fades never reach it either: they upload 510 bytes from
        // entry 0, which is entries 0-169.
        run_ticks(&mut rig, &mut part, FADE_OUT_FIRST_TICK + FADE_OUT_STEPS - SECOND_WHITE_TICK - 1);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "no fade step writes 255");
    }

    #[test]
    fn the_opening_screen_is_the_shells_surface_through_the_shells_dac() {
        // `END.C` has no `memset`: video memory at the part's first tick is
        // whatever `inittwk`'s `rep stosw` left — index 0 everywhere — and the
        // DAC is the one the shell's mode set installed, whose entry 0 is
        // black. That is why the reference's 430 is **index 0** in all 128,000
        // pixels and white only because `:62-63` has by then written 63 into
        // entry 0.
        let (mut rig, mut part) = started();
        // Tick 0 is `:61`'s blank: nothing has been written yet.
        part.frame(&mut rig.ctx(0));
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0));
        assert_eq!(rig.palette.entry(0), [0, 0, 0], "the shell's entry 0 is black");
        // Tick 1 is `:62-63`, and from there the white screen is the DAC's.
        part.frame(&mut rig.ctx(1));
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0));
    }

    #[test]
    fn the_picture_is_blitted_after_the_vertical_blanks() {
        // `:92`'s 32 blanks come before `:94-99`, and `:61` and `:89` add one
        // each — 34 in all — so video memory is still `inittwk`'s index 0 at
        // the reference's 430 (tick 26). The indexed comparison at 430 is what
        // makes this load-bearing: entry 0 is white by then, so the *pixels*
        // would match either way, while the `.idx` would not.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 430 - FIRST_INDEX);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "430 is index 0");

        // Through the last blank, and then `:94-99`.
        run_ticks(&mut rig, &mut part, BLIT_TICK - (430 - FIRST_INDEX));
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "still white at the last blank");
        part.frame(&mut rig.ctx(BLIT_TICK));
        assert_eq!(&rig.surface.row(0)[..PICTURE_WIDTH], &part.picture[..PICTURE_WIDTH]);
        assert_eq!(
            &rig.surface.row(PICTURE_HEIGHT - 1)[..PICTURE_WIDTH],
            &part.picture[(PICTURE_HEIGHT - 1) * PICTURE_WIDTH..]
        );
        // The blit is the source's `lineblit` at the surface's own stride: the
        // 320-wide view reads the left half of each 640-wide row back.
        assert!(rig.surface.row(0)[PICTURE_WIDTH..].iter().all(|pixel| *pixel == 0));

        // The lead-in's shape, in the port's own numbering, so that the counts
        // the note and A7 quote are arithmetic on asserted numbers rather than
        // prose of their own: four tick numbers, and the gap between the blit
        // and the fade.
        //
        // 34 vertical blanks — `:61` (tick 0), `:89` (tick 2) and `:92`'s 32
        // from tick 4 — and three ticks the code between them runs on: `:62-63`
        // with `:67-88` (tick 1), `:90-91` (tick 3) and `:94-99` (tick 36).
        assert_eq!([FIRST_WAIT_TICK, SECOND_WAIT_TICK, BLANKS_FIRST_TICK], [0, 2, 4]);
        assert_eq!(BLANKS_TICKS, 32);
        assert_eq!(2 + BLANKS_TICKS, 34, "vertical blanks before the fade");
        assert_eq!(BLIT_TICK, BLANKS_FIRST_TICK + BLANKS_TICKS, "the blit is the tick after the last blank");
        assert_eq!(BLIT_TICK, 36, "the tick `:94-99` runs on");
        assert_eq!(2 + BLANKS_TICKS + 3, 37, "ticks the source's own code accounts for");
        // And the rest, through to the fade's first displayed step (tick 47,
        // index 451): the blit's own tick and the ten the port does not model.
        // Written as additions, so that a BLIT_TICK past the fade's own is a
        // failed assertion rather than a subtraction that overflows the build.
        assert_eq!(BLIT_TICK + 1, FADE_IN_FIRST_TICK - 10);
        assert_eq!(BLIT_TICK + 11, FADE_IN_FIRST_TICK, "the blit's tick and ten unmodelled ones");
        assert_eq!(FIRST_INDEX + BLIT_TICK, 440);
        assert_eq!(FIRST_INDEX + FADE_IN_FIRST_TICK, FADE_IN_FIRST_INDEX);
    }

    #[test]
    fn the_fade_in_ramp_is_white_to_picture_and_truncates_where_the_original_truncates() {
        // ((128-c)*63 + pal*c)/128, integer division, over 6-bit entries. A
        // float version agrees on c = 0 and c = 128 and disagrees in the last
        // third.
        let mut pal = Palette::new();
        pal.set_entry(0, [63, 40, 1]);
        assert_eq!(fade_in_step(&pal, 1).entry(0)[0], ((127 * 63 + 63) / 128) as u8);
        assert_eq!(fade_in_step(&pal, 64).entry(0)[0], ((64 * 63 + 63 * 64) / 128) as u8);
        assert_eq!(fade_in_step(&pal, 1).entry(0)[1], ((127 * 63 + 40) / 128) as u8);
        assert_eq!(fade_in_step(&pal, 128).entry(0), [63, 40, 1], "c = 128 is the picture");
        assert_eq!(fade_in_step(&pal, 0).entry(0), [63, 63, 63], "c = 0 is white");
        // The first step, where truncation is already visible: entry 0's blue
        // is 1, and (127*63 + 1*1)/128 = 62.5 truncates to 62 where a rounded
        // ramp would leave it at 63.
        assert_eq!(fade_in_step(&pal, 1).entry(0)[2], 62);
    }

    #[test]
    fn the_fade_out_ramp_is_picture_to_black_and_is_not_the_fade_in_reversed() {
        // `:114` is (pal*c)/64 — from the picture at c = 64 down to black at
        // c = 0 — while the fade-in comes *from* white. The two ramps share
        // their end points with the picture's palette and nothing else, which
        // is why the brief's name for this test ("the same ramp reversed")
        // would be a wrong description of two correct lines of source.
        let mut pal = Palette::new();
        pal.set_entry(0, [63, 40, 1]);
        assert_eq!(fade_out_step(&pal, 0).entry(0), [0, 0, 0], "c = 0 is black");
        assert_eq!(fade_out_step(&pal, 64).entry(0), [63, 40, 1], "c = 64 is the picture");
        assert_eq!(fade_out_step(&pal, 63).entry(0)[2], 63 / 64);
        // The loop starts at 63, so the first displayed step is one below the
        // picture: the fade-in's c = 128 is the palette itself and the
        // fade-out's c = 63 is not.
        assert_ne!(fade_out_step(&pal, 63).entry(0), pal.entry(0));
        assert_eq!(
            fade_out_step(&pal, 63).entry(0)[0],
            (63u32 * 63 / 64) as u8,
            "truncated, not rounded"
        );
    }

    #[test]
    fn the_upload_is_the_originals_two_count_plus_start_bytes() {
        // `END/ASM.ASM:106-108` sets the transfer length to 2*count + start,
        // not 3*count: `setpalarea(ramp, 0, 255)` moves 510 bytes — entries
        // 0-169 whole — where it means to move 765. Entries 170-255 keep
        // whatever they held (docs/fidelity/bugs.md §4).
        let mut ramp = Palette::new();
        for index in 0u8..=255 {
            ramp.set_entry(index, [index % 64, 5, 6]);
        }
        let mut dac = Palette::new();
        dac.set_entry(169, [9, 9, 9]);
        dac.set_entry(170, [1, 2, 3]);
        dac.set_entry(254, [4, 5, 6]);
        dac.set_entry(255, [7, 8, 9]);
        setpalarea(&mut dac, &ramp, 0, 255);

        assert_eq!(dac.entry(0), ramp.entry(0));
        assert_eq!(dac.entry(169), ramp.entry(169), "the last entry written whole");
        assert_eq!(dac.entry(170), [1, 2, 3], "the 511th byte is never written");
        assert_eq!(dac.entry(254), [4, 5, 6]);
        assert_eq!(dac.entry(255), [7, 8, 9], "this routine never writes 255");
        // 510 and not 508: the call passes 255 where BEGLOGO's passes 254, and
        // the routine doubles whatever it is given.
        assert_eq!(2 * SETPALAREA_ENTRIES, 510);
        // The byte count is a *byte* count: with count = 254 only entry 169's
        // red byte lands, which is the state BEGLOGO leaves its DAC in.
        let mut other = Palette::new();
        other.set_entry(169, [9, 9, 9]);
        setpalarea(&mut other, &ramp, 0, 254);
        assert_eq!(other.entry(169), [ramp.entry(169)[0], 9, 9]);
    }

    // --- the gate, and the check that it reproduces the measurement ----------

    #[test]
    fn the_gate_opens_on_the_tick_the_reference_frames_derived() {
        // **The phase's proof.** `:107-111` runs its own loop against a player: one
        // iteration a tick, `break` when `dis_musplus()>-16`. What this test drives it with
        // is `reference/music/second5`'s own published words, and what it asserts is that
        // the gate opens where Phase 2 derived — bit-exactly, from the reference's
        // **frames**, which are a different measurement entirely: the picture's own fade
        // constants (`docs/fidelity/endlogo.md` §3). They meet on the tick §3 fixes once
        // the three wires below are in place — two of them this part's, one of them the
        // aligned frame — and that is the whole of the comparison:
        //
        //   the words cross          index 1144   (the port's player: row 49)
        //   the check's own frame    index 1145   (the source's placement: the check reads
        //                                          the words the tick before published)
        //   the aligned frame        index 1146   ([`GATE_ALIGNMENT_TICKS`]: the one frame
        //                                          `second5`'s session and the frames'
        //                                          session are apart)
        //   the fade-out's `c = 63`  index 1147   (1147 − 580 = 567 ticks, the hold)
        //
        // The words cross the threshold on exactly one index, and the trace says which:
        // `zplus` is 1 through MUSIC0's order 26 (its successor at 27 is a `254`), so
        // `musplus` is `row - 64` — **-16** at row 48, which is *not* `> -16`, and -15 from
        // row 49 on. `second5` holds row 48 at 1136…1144 and row 49 from 1145; its frame
        // 1145 is the port's index 1144 (see [`Player::trace_second5`]).
        let player = Player::trace_second5();
        assert_eq!(player.reading_at(1143), musplus_from(1, 48), "the index before the crossing");
        assert_eq!(player.reading_at(1143), GATE_THRESHOLD, "…and it is exactly the threshold");
        assert_eq!(player.reading_at(1144), musplus_from(1, 49), "the crossing, at the port's 1144");

        // The fade-in's last tick is 175 and the gate's first is 176, so the break's frame
        // is the tick whose count is 1145 − 580 = 565: run the part to it and the check has
        // just broken — the phase says so, and the DAC has not moved.
        let (mut rig, mut part) = started_with(Rig::with_player(player));
        let break_tick = FADE_OUT_FIRST_TICK - 2;
        assert_eq!(break_tick, 741, "the break's tick, index 1145");
        run_ticks(&mut rig, &mut part, break_tick + 1);
        assert_eq!(
            part.phase,
            Phase::Gate { count: break_tick - (FADE_IN_FIRST_TICK + FADE_IN_STEPS), held: 1 },
            "the break is detected on its own frame, and the alignment has begun"
        );
        assert_eq!(dac_entries(&rig), settled(&part), "index 1145 is still the held picture");

        // The aligned frame, still the held picture, and then the fade-out's `c = 63` — the
        // first step below the picture, which is what the reference's frame at 1180 counts
        // back from (1180 − 33 = 1147).
        run_ticks(&mut rig, &mut part, (FADE_OUT_FIRST_TICK - 1) - break_tick);
        assert_eq!(dac_entries(&rig), settled(&part), "index 1146 is the aligned frame");
        let status = part.frame(&mut rig.ctx(FADE_OUT_FIRST_TICK));
        assert_eq!(status, PartStatus::Continuing);
        assert_eq!(rig.palette.entry(1), fade_out_step(&part.picture_palette, 63).entry(1));
        assert_eq!(rig.palette.entry(1), [47, 44, 41]);
        assert_ne!(rig.palette.entry(1), part.picture_palette.entry(1));

        // The held length the cue answered with, against the one the reference's frames
        // give: 1147 − 580 = 567 ticks, from two different keyframes (500 and 1180).
        assert_eq!(HOLD_TICKS, 567);
        assert_eq!(HOLD_FIRST_INDEX + HOLD_TICKS, FADE_OUT_FIRST_INDEX);
        assert_eq!(FIRST_INDEX + FADE_OUT_FIRST_TICK, FADE_OUT_FIRST_INDEX);
    }

    #[test]
    fn the_gate_holds_for_five_thousand_ticks_when_the_cue_never_comes() {
        // `:107` is `for(a=0;a<5000 && !dis_exit();a++)`: five thousand is the loop's own
        // bound, and the port keeps it. A player that never crosses the threshold — the
        // sentinel every tick — leaves the part holding exactly that long and then
        // fading out, which is the behaviour a run whose music never reaches the gate's
        // window gets, and the only length in this part that is not the music's.
        assert_eq!(GATE_LIMIT, 5000);
        let (mut rig, mut part) = started();
        let last_hold = FADE_IN_FIRST_TICK + FADE_IN_STEPS + GATE_LIMIT - 1;
        run_ticks(&mut rig, &mut part, last_hold + 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {last_hold}: the last of the five thousand");
        // The loop's `a++` takes `a` to 5000 and the loop is left; the tick after the
        // last iteration is the fade-out's first step.
        part.frame(&mut rig.ctx(last_hold + 1));
        assert_eq!(rig.palette.entry(1), fade_out_step(&part.picture_palette, 63).entry(1));
        // Which is 5000 ticks and not 5001: the cap is on iterations, and the break's
        // own tick is one of them.
        assert_eq!(
            last_hold + 1 - (FADE_IN_FIRST_TICK + FADE_IN_STEPS),
            GATE_LIMIT,
            "the hold is the loop's bound"
        );
    }

    #[test]
    fn the_gates_check_reads_the_previous_ticks_words() {
        // **The one frame the two measurements are apart by, held still.**
        //
        // The source's check is the instruction after `dis_waitb()` returns, so it stands
        // at the frame boundary — and the copper's poll is a fifth of a frame *later*
        // (`copper_doit` is the list's scanline-20 entry, `MAIN/COPPER.ASM:41-43`, and
        // `_zpollme` is `:466` inside it), so the words it reads are the previous frame's.
        // The port's engine polls before the part runs in the same tick, so the part's own
        // `musplus()` at tick `T` is the words the source's check at the boundary into the
        // *next* tick's frame reads: the check tests the tick before.
        //
        // Two scripts make that a measurement rather than prose. Both cross at the first
        // gate tick — `musplus` is -15 at every tick, the smallest row that crosses — and
        // where the part starts fading says which tick's words the check read.
        let (mut rig, mut part) = started_with(Rig::with_player(Player::sentinel()));
        // The sentinel player is scripted by `at`, so this rig's own script is set after
        // `init`: every tick from the join on answers -15 (`zplus` 1, row 49).
        let crossing =
            vec![(1, 49); (FADE_IN_FIRST_TICK + FADE_IN_STEPS + 2) as usize];
        rig.player = Player::scripted(crossing);

        // The gate's first iteration is the tick after the fade-in's last, index 580, and
        // the words it reads are the tick before's — the fade-in's last tick. They crossed
        // there, so the gate breaks at once: the tick the break is drawn on is still the
        // held picture (the source's `break` follows its own `dis_waitb()`), the tick after
        // it is the frame the port holds past the break ([`GATE_ALIGNMENT_TICKS`]), and the
        // *third* tick is the fade-out's first step.
        let first_gate_tick = FADE_IN_FIRST_TICK + FADE_IN_STEPS;
        run_ticks(&mut rig, &mut part, first_gate_tick + 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {first_gate_tick} is the break's own frame");
        assert_eq!(
            part.phase,
            Phase::Gate { count: 0, held: 1 },
            "the break is detected on the first iteration, and the alignment has begun"
        );
        run_ticks(&mut rig, &mut part, 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {} is the aligned frame", first_gate_tick + 1);
        part.frame(&mut rig.ctx(first_gate_tick + 2));
        assert_eq!(
            rig.palette.entry(1),
            fade_out_step(&part.picture_palette, 63).entry(1),
            "the tick after the alignment is the fade-out's `c = 63`"
        );
        // One tick of hold for the break and one for the alignment: the check that broke
        // was the very first iteration's, and the iteration's frame is the hold's first
        // (index 580).
        assert_eq!(FIRST_INDEX + first_gate_tick, HOLD_FIRST_INDEX, "the gate's first iteration is index 580");

        // And the crossing the script carries is one above the threshold — `row - 64` at
        // `zplus` 1 — which is what the gate's own comparison is about.
        assert_eq!(musplus_from(1, 49), GATE_THRESHOLD + 1);
        assert!(musplus_from(1, 48) <= GATE_THRESHOLD, "…and one row below it does not cross");
    }

    #[test]
    fn a_run_with_no_player_is_refused_by_name() {
        // A part that waits on a cue cannot be handed nothing: `init` says so, and names
        // itself, rather than letting a run hold five thousand ticks against a silence
        // that cannot break (`src/parts/endlogo.rs`'s `init`, `PartCtx::music`).
        let mut rig = Rig::new();
        let mut part = Endlogo::at("../../assets/END/PIC.UH");
        let error = part.init(&mut rig.ctx_without_music(0)).unwrap_err();
        assert_eq!(error.part, NAME);
        assert!(error.message.contains("107-111"), "{}", error.message);
        assert!(error.message.contains("no music"), "{}", error.message);
    }

    #[test]
    fn the_hold_is_the_cues_own_length_and_not_five_thousand() {
        // The hold as the reference's frames have it, walked tick by tick with the
        // trace's words: the settled picture from the tick after the fade-in's last step
        // (index 580) until the fade-out's first (index 1147), 567 ticks — not the
        // loop's 5000-iteration bound, and not something the port chose. The check is
        // [`the_gate_opens_on_the_tick_the_reference_frames_derived`]; this walks the
        // whole hold instead of its ends.
        let (mut rig, mut part) = started_with(Rig::with_player(Player::trace_second5()));
        // The tick before the hold's first: the fade-in's c = 128, which is the
        // picture's own palette.
        run_ticks(&mut rig, &mut part, FADE_IN_FIRST_TICK + FADE_IN_STEPS);
        let settled = settled(&part);
        for (index, entry) in settled.iter().enumerate() {
            assert_eq!(*entry, part.picture_palette.entry(index as u8), "entry {index} at c = 128");
        }

        // Through the hold, the DAC does not move — and the last tick of it is the one
        // whose check breaks.
        for tick in FADE_IN_FIRST_TICK + FADE_IN_STEPS..FADE_OUT_FIRST_TICK {
            let status = part.frame(&mut rig.ctx(tick));
            assert_eq!(status, PartStatus::Continuing, "tick {tick} is inside the hold");
            assert_eq!(dac_entries(&rig), settled, "tick {tick} moved the DAC");
        }
        // The number of ticks walked, as arithmetic on the two constants the
        // reference's frames fixed.
        assert_eq!(FADE_OUT_FIRST_TICK - (FADE_IN_FIRST_TICK + FADE_IN_STEPS), HOLD_TICKS);
    }

    #[test]
    fn the_runs_own_player_opens_the_gate_on_the_same_tick() {
        // **Step 2's check, where the cue is the port's own.** The test above drives the
        // gate with the trace's words; this one drives it through a real run — the same
        // entry point `--part endlogo` and `--block 5` begin at, the run's own anchor and
        // numbering, the real asset, and `src/audio.rs`'s player. Together they are the
        // phase's proof: the port's cue opens the gate on the tick Phase 2 derived from the
        // reference's *frames* (`docs/fidelity/endlogo.md` §3), and the words it opens it
        // on are the trace's (`src/shell.rs`'s
        // `the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has`).
        //
        // What the run's frames are compared against is the derivation's own arithmetic,
        // not `reference/`: the four keyframes are the integration test's business, and
        // this test's is the *tick* — with a real player, the only thing that can move it
        // is the music.
        let plan = vec!["endlogo"];
        let mut run = crate::shell::Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            View::new(VIEW_WIDTH, VIEW_HEIGHT),
            plan,
            |name| {
                (name == NAME)
                    .then(|| Box::new(Endlogo::at("../../assets/END/PIC.UH")) as Box<dyn Part>)
            },
            Some(crate::audio::test_player),
            crate::shell::RunSettings::default(),
        )
        .unwrap();
        assert_eq!(run.anchor(), Some(FIRST_INDEX), "the run's indices are the reference's");
        assert_eq!(
            run.music_start_frame(),
            Some(crate::shell::MUSIC_START_FRAME),
            "and its clock starts where the music's own does"
        );

        // Run to the fade-out's first index, noting the published position at the hold's
        // two ends — the two readings the report carries, taken from the port's player
        // rather than from the stub.
        let mut ends: Vec<(u32, String)> = Vec::new();
        while run.index() < FADE_OUT_FIRST_INDEX {
            assert_ne!(
                run.tick(),
                SequenceStatus::Finished,
                "the run stopped at index {} without the gate opening",
                run.index()
            );
            let index = run.index() - 1;
            if index == HOLD_FIRST_INDEX || index + 1 == FADE_OUT_FIRST_INDEX {
                let position = run.music_position().expect("this run has a player");
                ends.push((index, format!("{}:{}", position.order, position.row)));
            }
        }

        // The tick the gate broke on: the held picture, from the part's own arithmetic —
        // the fade-in's last step, which is the picture's own palette.
        let image = picture();
        let held = rendered(image.pixels(), &dac_after_fade_in(&image.palette, FADE_IN_STEPS - 1));
        assert_eq!(
            render(run.frame()),
            held,
            "index {} is the held picture",
            FADE_OUT_FIRST_INDEX - 1
        );

        // And the tick after it is the fade-out's `c = 63`, the step the reference's 1180
        // counts 33 steps back from.
        assert_ne!(run.tick(), SequenceStatus::Finished, "the fade-out draws");
        assert_eq!(run.index() - 1, FADE_OUT_FIRST_INDEX);
        let first_step = rendered(image.pixels(), &dac_after_fade_out(&image.palette, FADE_OUT_STEPS - 1));
        assert_eq!(render(run.frame()), first_step, "index {FADE_OUT_FIRST_INDEX} is `c = 63`");

        // The hold the cue produced, in the run's own numbering: 1147 − 580 = 567 ticks,
        // the number the reference's frames give.
        assert_eq!(FADE_OUT_FIRST_INDEX - HOLD_FIRST_INDEX, HOLD_TICKS);
        // And the two ends' published positions are the trace's own, as the place in the
        // music that the run's clock and the trace's frames meet.
        assert_eq!(ends.len(), 2, "both ends of the hold were visited");
        for (index, position) in &ends {
            let (trace, _) = trace_at(*index);
            assert_eq!(
                *position, trace,
                "the run's position at index {index} is not the trace's"
            );
        }
    }

    /// The composed frame as RGB, the way a dump writes it: `present` under the dump's own
    /// options — one output pixel a source pixel, no border, no scaling.
    fn render(frame: &sr_engine::IndexedFrame) -> Vec<u8> {
        use sr_engine::present::{present, Aspect, FrameBuffer, PresentOptions};
        let options = PresentOptions { aspect: Aspect::SquarePixels, scale: 1, border: false };
        let mut buffer = FrameBuffer::new(0, 0);
        present(frame, &options, &mut buffer);
        let mut rgb = Vec::with_capacity(buffer.width() * buffer.height() * 3);
        for rgba in buffer.rgba().chunks_exact(4) {
            rgb.extend_from_slice(&rgba[..3]);
        }
        rgb
    }

    #[test]
    fn the_part_retires_after_the_fade_out_has_finished() {
        // `:112`'s loop is the last thing `main()` does: `:118-129`'s epilogue
        // is commented out and `:130`'s `}` returns. The last frame is the
        // fade-out's c = 0 — all black, since entries 0-169 are what the
        // picture indexes — and it is drawn on tick 806, index 1210.
        assert_eq!(FIRST_INDEX + FADE_OUT_FIRST_TICK + FADE_OUT_STEPS - 1, PART_LAST_INDEX);
        assert_eq!(PART_LAST_INDEX, 1210);

        let (mut rig, mut part) = started_with(Rig::with_player(Player::trace_second5()));
        let last = FADE_OUT_FIRST_TICK + FADE_OUT_STEPS - 1;
        run_ticks(&mut rig, &mut part, last);
        assert_eq!(part.frame(&mut rig.ctx(last)), PartStatus::Continuing, "the last step draws");
        assert_eq!(rig.palette.entry(0), [0, 0, 0], "c = 0 is black");
        assert_eq!(
            part.frame(&mut rig.ctx(last + 1)),
            PartStatus::Finished,
            "the tick after the loop is past `main()`"
        );
    }

    // --- the derivation, run over the reference's own frames ----------------

    /// The DAC as it stands after fade-in step `c`: `:62-63`'s white writes,
    /// then steps `0..=c` of the ramp through the shipped [`setpalarea`].
    ///
    /// The reference side of the scan below, and the reason it can be a scan:
    /// the whole DAC is rebuilt from the part's own arithmetic rather than
    /// adjusted step by step, so `c` is the only thing that varies.
    fn dac_after_fade_in(picture: &Palette, c: u32) -> Palette {
        fade_in_dac(picture, c, SETPALAREA_ENTRIES)
    }

    /// [`dac_after_fade_in`] with `count` in place of the source's 255: with
    /// 254 the upload moves 508 bytes, which is BEGLOGO's own call.
    fn fade_in_dac(picture: &Palette, c: u32, count: u32) -> Palette {
        let mut dac = white();
        for step in 0..=c {
            let ramp = fade_in_step(picture, step);
            setpalarea(&mut dac, &ramp, 0, count);
        }
        dac
    }

    /// The same for the fade-out: step `c` and every step it ran after, from
    /// the fade-out's own `c = 63` down. The fade-in's DAC is deliberately not
    /// the starting point: the fade-out's first step writes entries 0-169
    /// wholly, so the fade-in's state of those entries cannot reach the frame
    /// that step displays.
    fn dac_after_fade_out(picture: &Palette, c: u32) -> Palette {
        let mut dac = white();
        for step in (c..=FADE_OUT_STEPS - 1).rev() {
            setpalarea(&mut dac, &fade_out_step(picture, step), 0, SETPALAREA_ENTRIES);
        }
        dac
    }

    /// The DAC `:90-91` leaves, with entry 255 unwritten: the reference's own
    /// 430 is index 0, so nothing in these scans reads it.
    fn white() -> Palette {
        let mut dac = Palette::new();
        for entry in 0..=254u8 {
            dac.set_entry(entry, [63, 63, 63]);
        }
        dac
    }

    /// What the frame would be: every pixel of the picture through `dac`, with
    /// the DAC's own 6→8 widening.
    fn rendered(picture: &[u8], dac: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(picture.len() * 3);
        for pixel in picture {
            let entry = dac.entry(*pixel);
            out.push(sr_engine::dac_expand(entry[0]));
            out.push(sr_engine::dac_expand(entry[1]));
            out.push(sr_engine::dac_expand(entry[2]));
        }
        out
    }

    fn reference(name: &str) -> Vec<u8> {
        std::fs::read(format!("../../reference/parts/endlogo/{name}")).unwrap()
    }

    fn picture() -> sr_assets::Image {
        load_path(Path::new("../../assets/END/PIC.UH")).unwrap()
    }

    /// **The constants are a derivation, not a fit** (plan Review Focus 5).
    ///
    /// Each fade keyframe is run against the whole of its own ramp's range and
    /// asks which `c`, if any, reproduces its 320×400 RGB **exactly** — the
    /// source's arithmetic, the engine's own 6→8 widening, and the picture,
    /// pixel for pixel. One `c` answers each, so each keyframe names the index
    /// its step is displayed on uniquely, and `docs/fidelity/endlogo.md` §3
    /// carries the table. A fit over §19.1's sweep means puts the fade-in's
    /// step 0 "near 450"; the scan says 451, which is a different frame.
    #[test]
    fn the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit() {
        let image = picture();

        // The decode is the strongest statement available that the decoder and
        // the art agree: the reference's held frame is its indices, byte for
        // byte — 0 of 128,000 pixels different.
        let held = reference("frame_000900.idx");
        assert_eq!(
            held.len(),
            image.pixels().len(),
            "the reference's held frame is {} bytes where the decode is {}",
            held.len(),
            image.pixels().len()
        );
        let differing = image.pixels().iter().zip(&held).filter(|(a, b)| a != b).count();
        assert_eq!(
            differing, 0,
            "the decoded picture differs from the reference's held frame in {differing} of {} pixels",
            held.len()
        );

        // 430 is the white field — one index, 0, in all 128,000 pixels — which
        // is `inittwk`'s cleared video memory before the picture is blitted.
        let opening = reference("frame_000430.idx");
        assert_eq!(opening.len(), PICTURE_WIDTH * PICTURE_HEIGHT);
        assert!(
            opening.iter().all(|index| *index == 0),
            "430 is not the screen the shell's mode set left"
        );

        // The picture's own histogram, which two later claims rest on: it
        // indexes 0-71 and nothing else, so every index it uses is inside the
        // 170 entries the fade's 510-byte upload writes — and so the upload's
        // byte count cannot be read off any keyframe.
        let mut used = std::collections::BTreeSet::new();
        used.extend(image.pixels().iter().copied());
        assert_eq!(used.len(), 72, "the picture's distinct indices");
        assert_eq!(used.iter().next(), Some(&0));
        assert_eq!(used.iter().next_back(), Some(&71));

        // The fade-in: every `c` in 0…128 against 500's own bytes.
        let early = reference("frame_000500.rgb");
        let mut at_500 = Vec::new();
        let mut beglogo_upload = Vec::new();
        for c in 0..=128u32 {
            if rendered(image.pixels(), &dac_after_fade_in(&image.palette, c)) == early {
                at_500.push(c);
            }
            // The same scan with BEGLOGO's call — `setpalarea(pal2,0,254)`,
            // 508 bytes — which is the one neighbouring byte count this part
            // has, and which answers the same `c`.
            if rendered(image.pixels(), &fade_in_dac(&image.palette, c, 254)) == early {
                beglogo_upload.push(c);
            }
        }
        assert_eq!(at_500, [49], "500 is the fade-in's ramp at c = 49, uniquely");
        let fade_in_first = 500 - at_500[0];
        assert_eq!(fade_in_first, FADE_IN_FIRST_INDEX, "the fade-in's step 0 is displayed at {fade_in_first}");
        assert_eq!(beglogo_upload, at_500, "the upload's byte count is invisible on this art");

        // The fade-out: every `c` in 63…0 against 1180's own bytes. The loop
        // counts down, so the step displayed at 1180 is `63 - (1180 - first)`.
        let late = reference("frame_001180.rgb");
        let mut at_1180 = Vec::new();
        for c in (0..=63u32).rev() {
            if rendered(image.pixels(), &dac_after_fade_out(&image.palette, c)) == late {
                at_1180.push(c);
            }
        }
        assert_eq!(at_1180, [30], "1180 is the fade-out's ramp at c = 30, uniquely");
        let fade_out_first = 1180 - (FADE_OUT_STEPS - 1 - at_1180[0]);
        assert_eq!(
            fade_out_first, FADE_OUT_FIRST_INDEX,
            "the fade-out's step 63 is displayed at {fade_out_first}"
        );

        // 900 is past the fade-in's convergence — c = 126, 127 and 128 are the
        // same palette — so it pins the end and not the start. Resolved by the
        // picture, not by arithmetic.
        let settled = reference("frame_000900.rgb");
        let mut at_900 = Vec::new();
        for c in 0..=128u32 {
            if rendered(image.pixels(), &dac_after_fade_in(&image.palette, c)) == settled {
                at_900.push(c);
            }
        }
        assert_eq!(at_900, [126, 127, 128], "900 is converged, and only bounds the hold");

        // The two independent facts the plan asks the constants to be checked
        // against, in the tree: the hold is real and 567 ticks long, and the
        // 64-step fade-out ends inside (1200, 1240] — the window in which
        // §20's geometry change to CRED's mode 13h screen sits.
        assert_eq!(HOLD_FIRST_INDEX, fade_in_first + FADE_IN_STEPS);
        assert_eq!(HOLD_TICKS, fade_out_first - HOLD_FIRST_INDEX);
        assert_eq!(PART_LAST_INDEX, fade_out_first + FADE_OUT_STEPS - 1);
        const { assert!(PART_LAST_INDEX > 1200 && PART_LAST_INDEX <= 1240); }
        // And the port's own tick map lands on the two indices the reference's
        // frames fixed: the tick the source's fade-out loop starts on is the
        // tick 1147 is drawn on, and its last step is 1210's.
        assert_eq!(FIRST_INDEX + FADE_OUT_FIRST_TICK, FADE_OUT_FIRST_INDEX);
        assert_eq!(FIRST_INDEX + PART_LAST_TICK, PART_LAST_INDEX);
    }

    /// The record's own sweep samples, reproduced from the two constants —
    /// **evidence for 451 and 1147 that neither keyframe's scan could give.**
    ///
    /// `reference-indices.md` §19.1 lists what a 24-sample sweep of the whole
    /// part showed, as a colour count and a mean per sample, and the sweep
    /// documents its samples where they fall in the fade-in's 129 steps and the
    /// fade-out's 64: 460 nine steps in, 500 forty-nine, 540 eighty-nine, and
    /// 1160 fifty steps before the fade-out's end, 1200 ten. Every one of those
    /// numbers is a *prediction* of a constant derived from a single keyframe
    /// each, and the means are to two decimals over 384,000 bytes.
    ///
    /// The means are the record's, not the port's: the port's frame is the
    /// picture through the ramp, widened, and the arithmetic below is the same
    /// arithmetic the part runs. A constant that is one frame out moves every
    /// one of these numbers.
    #[test]
    fn the_sweeps_own_samples_are_the_ramps_frames() {
        let image = picture();
        let settled = |index: u32| -> Vec<u8> {
            if (HOLD_FIRST_INDEX..FADE_OUT_FIRST_INDEX).contains(&index) {
                // The hold: the picture through its own palette, the fade-in's
                // last step.
                rendered(image.pixels(), &dac_after_fade_in(&image.palette, FADE_IN_STEPS - 1))
            } else if index < FADE_IN_FIRST_INDEX {
                // Before the fade: the screen is one index (0) and the DAC is
                // white. Any index the picture does not use renders the same,
                // so this is the white field rather than the picture.
                rendered(
                    &vec![0u8; PICTURE_WIDTH * PICTURE_HEIGHT],
                    &dac_after_fade_in(&image.palette, 0),
                )
            } else if index < HOLD_FIRST_INDEX {
                let c = index - FADE_IN_FIRST_INDEX;
                rendered(image.pixels(), &dac_after_fade_in(&image.palette, c))
            } else {
                let c = FADE_OUT_STEPS - 1 - (index - FADE_OUT_FIRST_INDEX);
                rendered(image.pixels(), &dac_after_fade_out(&image.palette, c))
            }
        };

        // `(index, colours, mean in hundredths)` — §19.1's own table, its nine
        // sampled states of this part. Every one is a prediction of the two
        // constants.
        let sweep: [(u32, usize, i64); 9] = [
            (430, 1, 25500),
            (460, 16, 23744),
            (500, 56, 16731),
            (540, 66, 9954),
            (580, 69, 3323),
            (900, 69, 3323),
            (1120, 69, 3323),
            (1160, 65, 2515),
            (1200, 30, 429),
        ];
        let mut held = None;
        for (index, colours, mean) in sweep {
            let frame = settled(index);
            let distinct = {
                let mut set = std::collections::BTreeSet::new();
                for triple in frame.chunks_exact(3) {
                    set.insert([triple[0], triple[1], triple[2]]);
                }
                set.len()
            };
            let hundredths =
                (frame.iter().map(|byte| *byte as f64).sum::<f64>() / frame.len() as f64 * 100.0)
                    .round() as i64;
            assert_eq!(distinct, colours, "{index}: colours against §19.1's table");
            assert_eq!(hundredths, mean, "{index}: mean against §19.1's table");
            // The record's "every sample from 580 to 1120 is the same sha256"
            // is the hold being one frame, which is what `settled` returns for
            // each of those three.
            if index == 580 {
                held = Some(frame);
            } else if matches!(index, 900 | 1120) {
                assert_eq!(Some(&frame), held.as_ref(), "{index} is not the held picture");
            }
        }
    }
}
