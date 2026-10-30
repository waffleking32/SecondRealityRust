//! ENDLOGO — the end picture: the FUTURE/CREW logo, white, faded in over a
//! held screen, held until the music's cue and faded out to black.
//!
//! `END/END.C` is the part, over `END/ASM.ASM` (`lineblit`, `setpalarea`, a
//! dead `inittwk`) and `END/READP.C` (BEGLOGO's row reader). One
//! `dis_waitb()` is one tick:
//!
//! ```text
//! init  :51-60, :67-88  AC unlock, display start 0, 400-line CRTC 9, AC 0x11 = 0
//! 0     :61             a blank: inittwk's index 0 through the shell's DAC
//! 1     :62-63          63 written 765 times: entries 0-254 white, 255 untouched
//! 2-3   :89, :90-91     a blank, and the same white again
//! 4-35  :92             32 blanks
//! 36    :94-99          readp and the 400 lineblit rows
//! 37-46                 not modelled (A7)
//! 47    :101-106        129 steps of white→picture, one a tick
//! ...   :107-111        the gate: until dis_musplus() > -16, at most 5000 ticks
//! ...   :112-117        64 steps of picture→black; :130 returns
//! ```
//!
//! The part sets no video mode (`:64-66` is commented out; `MAIN/U2.ASM:906-908`
//! sets mode 13h and runs `inittwk` before `exe2021`) and clears no video
//! memory, so the reference's 430 is index 0 under a white DAC. The gate is the
//! only variable-length act: 567 ticks on the reference's boot, as long as the
//! music says ([`Endlogo::gate`], [`GATE_ALIGNMENT_TICKS`]). The `!dis_exit()`
//! half of the gate is the shell's ESC.
//!
//! The picture, the 400-line view, the blit, the white-to-picture ramp and
//! `setpalarea` are [`logo`](super::logo)'s, shared with BEGLOGO. The tick map,
//! the fade constants' derivation and the gate's alignment are
//! `docs/fidelity/endlogo.md`; preserved bugs `bugs.md` §4.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::logo::{blit, load_picture, set_400_line_view, setpalarea, white_to_picture};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "endlogo";

/// The picture, read in its original format: `readp(palette,-1,pic)` and
/// `:95-99`'s 400 rows.
const PICTURE_PATH: &str = "assets/END/PIC.UH";

/// AC 0x11 after `:86-87` (the later of the 255 and 0 writes): the border
/// shows DAC entry 0, white until the fade-in carries it to black.
const OVERSCAN_ENTRY: u8 = 0;

/// The reference index of tick 0: the demo's first graphics frame, the
/// shell's anchor for `endlogo`.
const FIRST_INDEX: u32 = crate::shell::FIRST_GRAPHICS_FRAME;

/// `:61`'s `dis_waitb()`, before anything is written.
const FIRST_WAIT_TICK: u32 = 0;

/// `:62-63`'s white ([`white_dac_tick`]).
const WHITE_DAC_TICK: u32 = FIRST_WAIT_TICK + 1;

/// `:89`'s `dis_waitb()`.
const SECOND_WAIT_TICK: u32 = WHITE_DAC_TICK + 1;

/// `:90-91`: the white again, after the CRTC and AC writes.
const SECOND_WHITE_TICK: u32 = SECOND_WAIT_TICK + 1;

/// `:92`'s `for(a=0;a<32;a++) dis_waitb()`.
const BLANKS_TICKS: u32 = 32;

/// The first of `:92`'s blanks.
const BLANKS_FIRST_TICK: u32 = SECOND_WHITE_TICK + 1;

/// `:94-99`: the blit, on the tick after the last blank.
const BLIT_TICK: u32 = BLANKS_FIRST_TICK + BLANKS_TICKS;

/// The reference index the fade-in's `c = 0` is displayed on: 500 is the ramp
/// at `c = 49` and no other (`endlogo.md` §3).
const FADE_IN_FIRST_INDEX: u32 = 451;

/// [`FADE_IN_FIRST_INDEX`]'s tick, 47. The source accounts for 37 of the
/// ticks before it; the other ten are not modelled (A7).
const FADE_IN_FIRST_TICK: u32 = FADE_IN_FIRST_INDEX - FIRST_INDEX;

/// `:101`'s `for(c=0;c<=128;c++)`; `c = 128` is the picture's own palette.
const FADE_IN_STEPS: u32 = 129;

/// `:112`'s `for(c=63;c>=0;c--)`: it counts down ([`Endlogo::fade_out`]).
const FADE_OUT_STEPS: u32 = 64;

/// `:110`'s `dis_musplus()>-16`.
const GATE_THRESHOLD: i32 = -16;

/// `:107`'s `a<5000`, in iterations.
const GATE_LIMIT: u32 = 5000;

/// Ticks held past the gate's own break, so that the fade-out starts where
/// the reference's frames put it (1147, `endlogo.md` §3).
///
/// The port's player publishes row 49 from index 1144, so the check breaks on
/// 1145; `second5`'s session crossed one frame earlier than the session the
/// frames were captured in, and D12 makes the frames the record. The cost, one
/// frame against `second5` at this crossing, is inside A12's spread
/// (`endlogo.md` §5.2). It applies to the cue's break, not to [`GATE_LIMIT`].
const GATE_ALIGNMENT_TICKS: u32 = 1;

/// `musplus()` with no plus pending: `muscode_6`'s `-32` sentinel
/// (`DIS/DISINT.ASM:252`). Only the first tick's "previous" reading uses it;
/// `init` refuses a run with no player.
const NO_PLAYER: i32 = -32;

/// `:105`/`:116`'s `setpalarea(pal2,0,255)`: 510 bytes under the routine's
/// `2*count+start` bug ([`setpalarea`]).
const SETPALAREA_ENTRIES: u32 = 255;

/// The picture and the palette it indexes, both as `readp` leaves them.
pub struct Endlogo {
    /// The decoded picture, 320×400. Empty until `init`.
    picture: Vec<u8>,
    /// The container's palette (`END.C`'s `palette[]`): both fades' source.
    picture_palette: Palette,
    /// The last `dis_musplus()` reading, which the next tick's gate tests.
    plus: i32,
    phase: Phase,
}

/// `main()`'s three loops, as states: the lead-in is fixed by the source, the
/// gate's length is the music's.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Phase {
    /// `:61-106`, driven by `ctx.frame`.
    LeadIn,
    /// `:107-111`. `count` is the source's `a`; `held` counts the ticks held
    /// past the break ([`GATE_ALIGNMENT_TICKS`]), 0 while the gate is open.
    Gate { count: u32, held: u32 },
    /// `:112-117`. The displayed `c` is `FADE_OUT_STEPS - 1 - step`.
    FadeOut { step: u32 },
    /// `:130`: `main()` has returned.
    Done,
}

impl Endlogo {
    /// A part with no picture: `init` is what reads one.
    pub fn new() -> Endlogo {
        Endlogo {
            picture: Vec::new(),
            picture_palette: Palette::new(),
            plus: NO_PLAYER,
            phase: Phase::LeadIn,
        }
    }
}

impl Part for Endlogo {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `END.C:51-99` minus everything the part times: the AC unlock (PAS = 1,
    /// which the engine always displays), `:67-88`'s CRTC and overscan, and the
    /// picture read. They cost no frame; `:61`'s blank is [`FIRST_WAIT_TICK`].
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:107-111` has nothing but the player to wait on.
        ctx.require_music(
            NAME,
            "this part waits on the music player's position (`END/END.C:107-111`) \
             and the run has no music to wait on",
        )?;
        set_400_line_view(ctx, OVERSCAN_ENTRY);
        (self.picture, self.picture_palette) = load_picture(NAME, PICTURE_PATH)?;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // Read every tick: the gate's first check tests the reading of the
        // tick before it.
        let reading = match ctx.music() {
            Some(music) => music.musplus(),
            None => NO_PLAYER,
        };
        let previous = self.plus;
        self.plus = reading;

        match self.phase {
            Phase::LeadIn => self.lead_in(ctx),
            Phase::Gate { count, held } => self.gate(previous, count, held),
            Phase::FadeOut { step } => self.fade_out(ctx, step),
            // `:118-129` is commented out and `:130` ends `main()`.
            Phase::Done => PartStatus::Finished,
        }
    }
}

impl Endlogo {
    /// `:61-106`: the blanks, the two white writes, the blit, the ten
    /// unmodelled ticks and the 129-step fade-in.
    fn lead_in(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let tick = ctx.frame;
        match tick {
            FIRST_WAIT_TICK | SECOND_WAIT_TICK => PartStatus::Continuing,
            WHITE_DAC_TICK | SECOND_WHITE_TICK => white_dac_tick(ctx),
            tick if tick < BLIT_TICK => PartStatus::Continuing,
            BLIT_TICK => {
                blit(&self.picture, ctx);
                PartStatus::Continuing
            }
            // A7's ten ticks: the screen is white because the DAC is.
            tick if tick < FADE_IN_FIRST_TICK => PartStatus::Continuing,
            tick => {
                // `:103-105`.
                let step = tick - FADE_IN_FIRST_TICK;
                let ramp = white_to_picture(&self.picture_palette, step);
                setpalarea(ctx.palette, &ramp, 0, SETPALAREA_ENTRIES);
                if step + 1 == FADE_IN_STEPS {
                    self.phase = Phase::Gate { count: 0, held: 0 };
                }
                PartStatus::Continuing
            }
        }
    }

    /// `:107-111`: `for(a=0;a<5000 && !dis_exit();a++) { dis_waitb();
    /// if(dis_musplus()>-16) break; }`, one iteration a tick over the held
    /// fade-in's last step.
    ///
    /// The check tests the *previous* tick's reading. The source's check runs
    /// as `dis_waitb()` returns on the copper's frame boundary (`waitb_1`,
    /// `DIS/DISINT.ASM:161-181`, on the count `MAIN/COPPER.ASM:320` bumps),
    /// while the poll (`_zpollme`, `:466`) runs a fifth of a frame later in
    /// `copper_doit` (`copperlst0`, `:41-43`); the port's engine polls before
    /// the part. The break is then held [`GATE_ALIGNMENT_TICKS`] longer
    /// (`endlogo.md` §5).
    fn gate(&mut self, previous: i32, count: u32, held: u32) -> PartStatus {
        if previous > GATE_THRESHOLD || held > 0 {
            // The break's own frame, then `held` frames, then `:112`'s `c = 63`.
            if held >= GATE_ALIGNMENT_TICKS {
                self.phase = Phase::FadeOut { step: 0 };
            } else {
                self.phase = Phase::Gate { count, held: held + 1 };
            }
        } else if count + 1 == GATE_LIMIT {
            self.phase = Phase::FadeOut { step: 0 };
        } else {
            self.phase = Phase::Gate { count: count + 1, held: 0 };
        }
        PartStatus::Continuing
    }

    /// `:112-117`: `for(c=63;c>=0;c--) { pal2 = (palette*c)/64; dis_waitb();
    /// setpalarea(pal2,0,255); }`, one step a tick.
    fn fade_out(&mut self, ctx: &mut PartCtx, step: u32) -> PartStatus {
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

/// `END.C:62-63` and `:90-91`: `outp(0x3c8,0)`, then `for(a=0;a<768-3;a++)
/// outp(0x3c9,63)`. 765 bytes are entries 0-254: **entry 255 keeps** what the
/// part before left (BEGLOGO's `BEG/BEG.C:25-34` writes it black). No frame
/// shows it: 430 is index 0 and the picture indexes 0-71.
fn white_dac_tick(ctx: &mut PartCtx) -> PartStatus {
    for entry in 0..=254u8 {
        ctx.set_palette_entry(entry, [63, 63, 63]);
    }
    PartStatus::Continuing
}

/// `END.C:114`: `pal2[a] = (palette[a]*c)/64`, truncated. Not the fade-in
/// reversed: that one comes from white, this one goes to black.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::load_image;
    use crate::parts::logo::{PICTURE_HEIGHT, PICTURE_WIDTH};
    use crate::parts::test_rig::{reference, repo, rgb_of, Reading, Rig, ScriptedMusic};
    use crate::shell::{VIEW_HEIGHT, VIEW_WIDTH};
    use sr_engine::music::musplus_from;

    /// A DAC entry no path through the part writes: a marker for "left alone".
    const UNWRITTEN: u8 = 255;

    // The reference's frames' map (`endlogo.md` §3), kept to check the cue's
    // answer against: the part itself has no hold length.

    /// The fade-out's `c = 63`: 1180 is `c = 30` and no other, 1180 − 33.
    const FADE_OUT_FIRST_INDEX: u32 = 1147;

    /// The first index of the settled picture.
    const HOLD_FIRST_INDEX: u32 = FADE_IN_FIRST_INDEX + FADE_IN_STEPS;

    /// The hold the reference's frames show, 567 ticks.
    const HOLD_TICKS: u32 = FADE_OUT_FIRST_INDEX - HOLD_FIRST_INDEX;

    const FADE_OUT_FIRST_TICK: u32 = FADE_OUT_FIRST_INDEX - FIRST_INDEX;

    /// The last frame, the fade-out's `c = 0`: 1210, inside the window
    /// (1200, 1240] in which `SECOND 5` changes to CRED's screen
    /// (`reference-indices.md` §20).
    const PART_LAST_INDEX: u32 = FADE_OUT_FIRST_INDEX + FADE_OUT_STEPS - 1;

    /// A player answering `musplus()` from `(zplus, row)` pairs, one a tick,
    /// and the sentinel past the script.
    fn scripted(script: Vec<(i32, i32)>) -> ScriptedMusic {
        ScriptedMusic::musplus(script.into_iter().map(|(zplus, row)| musplus_from(zplus, row)).collect())
            .past_end(Reading::default())
            .never_syncs("`END.C` never syncs: the part's one music call is `dis_musplus()`")
    }

    /// A player with no plus pending: `musplus()` is `-32` at every tick.
    fn sentinel() -> ScriptedMusic {
        scripted(Vec::new())
    }

    /// What the scripted player publishes at reference index `index`.
    fn reading_at(music: &ScriptedMusic, index: u32) -> i32 {
        music.reading_at((index - FIRST_INDEX) as usize).musplus
    }

    /// `reference/music/second5`'s words as the port's player publishes them.
    ///
    /// The trace samples frame `N` at its start (D17), so it holds the words of
    /// the poll during `N - 1`, which the port publishes on tick `N - 1`: entry
    /// `t` is the trace's frame `FIRST_INDEX + t + 1`. The trace's gaps carry
    /// the previous sample forward.
    fn trace_second5() -> ScriptedMusic {
        let path = repo("reference/music/second5/trace.csv");
        let text = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let mut script: Vec<(i32, i32)> = Vec::new();
        for line in text.lines().skip(1) {
            let fields: Vec<&str> = line.split(',').map(str::trim).collect();
            let field = |at: usize| -> i64 { fields[at].parse().unwrap_or_else(|error| panic!("{line}: {error}")) };
            let (frame, zplus, row) = (field(0) as u32, field(8) as i32, field(2) as i32);
            if frame < FIRST_INDEX + 1 {
                continue;
            }
            let at = (frame - FIRST_INDEX - 1) as usize;
            while script.len() < at {
                script.push(*script.last().unwrap_or(&(0, 0)));
            }
            script.push((zplus, row));
        }
        scripted(script)
    }

    /// The shell's screen and DAC (entry 0 black), with a marker in the entry
    /// the part never writes, and the sentinel player.
    fn shell_rig() -> Rig {
        Rig::new(VIEW_WIDTH, VIEW_HEIGHT).palette_entry(UNWRITTEN, [1, 2, 3]).with(sentinel())
    }

    fn started() -> (Rig, Endlogo) {
        started_with(shell_rig())
    }

    fn started_with(mut rig: Rig) -> (Rig, Endlogo) {
        let mut part = Endlogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    #[test]
    fn the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left() {
        // U2E leaves `Some(320)`; on the 640-wide surface that would read
        // every second scanline from the picture's right margin (`endlogo.md`
        // §2).
        let mut rig = shell_rig();
        rig.view.y_line_offset = Some(320);
        let mut part = Endlogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        assert_eq!(rig.view.y_line_offset, None);
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Endlogo, ticks: u32) {
        for at in 0..ticks {
            part.frame(&mut rig.ctx(at));
        }
    }

    /// The DAC's entries 0-169, every entry the picture or a fade writes.
    fn dac_entries(rig: &Rig) -> Vec<[u8; 3]> {
        (0..=169u8).map(|index| rig.palette.entry(index)).collect()
    }

    /// The DAC the settled picture leaves: the fade-in's last step.
    fn settled(part: &Endlogo) -> Vec<[u8; 3]> {
        (0..=169u8).map(|index| part.picture_palette.entry(index)).collect()
    }

    #[test]
    fn the_white_leaves_entry_255_unwritten() {
        let (mut rig, mut part) = started();
        let before = rig.palette.entry(UNWRITTEN);
        assert_ne!(before, [63, 63, 63], "the marker entry is not already white");

        run_ticks(&mut rig, &mut part, WHITE_DAC_TICK + 1);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "`:63`'s 765 bytes stop short");

        run_ticks(&mut rig, &mut part, SECOND_WHITE_TICK + 1 - WHITE_DAC_TICK - 1);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "`:91`'s 765 bytes stop short too");

        // The fades upload 510 bytes from entry 0: entries 0-169.
        run_ticks(&mut rig, &mut part, FADE_OUT_FIRST_TICK + FADE_OUT_STEPS - SECOND_WHITE_TICK - 1);
        assert_eq!(rig.palette.entry(UNWRITTEN), before, "no fade step writes 255");
    }

    #[test]
    fn the_opening_screen_is_the_shells_surface_through_the_shells_dac() {
        // No `memset`: the surface is `inittwk`'s index 0, and 430 is white
        // only because `:62-63` wrote entry 0.
        let (mut rig, mut part) = started();
        part.frame(&mut rig.ctx(0));
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0));
        assert_eq!(rig.palette.entry(0), [0, 0, 0], "the shell's entry 0 is black");
        part.frame(&mut rig.ctx(1));
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0));
    }

    #[test]
    fn the_picture_is_blitted_after_the_vertical_blanks() {
        // 34 blanks before `:94-99`, so 430 (tick 26) is still index 0: the
        // pixels would be white either way, the `.idx` would not.
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, 430 - FIRST_INDEX);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "430 is index 0");

        run_ticks(&mut rig, &mut part, BLIT_TICK - (430 - FIRST_INDEX));
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 0), "still white at the last blank");
        part.frame(&mut rig.ctx(BLIT_TICK));
        assert_eq!(&rig.surface.row(0)[..PICTURE_WIDTH], &part.picture[..PICTURE_WIDTH]);
        assert_eq!(
            &rig.surface.row(PICTURE_HEIGHT - 1)[..PICTURE_WIDTH],
            &part.picture[(PICTURE_HEIGHT - 1) * PICTURE_WIDTH..]
        );
        // `lineblit` at the surface's own stride: the right half stays 0.
        assert!(rig.surface.row(0)[PICTURE_WIDTH..].iter().all(|pixel| *pixel == 0));

        // The blit at index 440, then A7's ten unmodelled ticks before 451.
        assert_eq!(FIRST_INDEX + BLIT_TICK, 440);
        assert_eq!(BLIT_TICK + 11, FADE_IN_FIRST_TICK);
    }

    #[test]
    fn the_fade_out_ramp_is_picture_to_black_and_is_not_the_fade_in_reversed() {
        let mut pal = Palette::new();
        pal.set_entry(0, [63, 40, 1]);
        assert_eq!(fade_out_step(&pal, 0).entry(0), [0, 0, 0], "c = 0 is black");
        assert_eq!(fade_out_step(&pal, 64).entry(0), [63, 40, 1], "c = 64 is the picture");
        // The loop starts at 63: its first step is one below the picture,
        // truncated.
        assert_eq!(fade_out_step(&pal, 63).entry(0), [62, 39, 0]);
    }

    // --- the gate ------------------------------------------------------------

    #[test]
    fn the_gate_opens_on_the_tick_the_reference_frames_derived() {
        // The gate driven by `second5`'s words meets the fade-out the
        // reference's frames derive (`endlogo.md` §5):
        //
        //   1144  the port's published words cross (row 49, musplus -16 → -15)
        //   1145  the check breaks (it reads the previous tick's words)
        //   1146  the aligned frame (GATE_ALIGNMENT_TICKS)
        //   1147  the fade-out's c = 63
        let player = trace_second5();
        assert_eq!(reading_at(&player, 1143), GATE_THRESHOLD, "row 48 is exactly the threshold");
        assert_eq!(reading_at(&player, 1144), musplus_from(1, 49), "the crossing, at the port's 1144");

        let (mut rig, mut part) = started_with(shell_rig().with(player));
        let break_tick = FADE_OUT_FIRST_TICK - 2;
        run_ticks(&mut rig, &mut part, break_tick + 1);
        assert_eq!(
            part.phase,
            Phase::Gate { count: break_tick - (FADE_IN_FIRST_TICK + FADE_IN_STEPS), held: 1 },
            "the break is detected on its own frame, and the alignment has begun"
        );
        assert_eq!(dac_entries(&rig), settled(&part), "index 1145 is still the held picture");

        run_ticks(&mut rig, &mut part, (FADE_OUT_FIRST_TICK - 1) - break_tick);
        assert_eq!(dac_entries(&rig), settled(&part), "index 1146 is the aligned frame");
        let status = part.frame(&mut rig.ctx(FADE_OUT_FIRST_TICK));
        assert_eq!(status, PartStatus::Continuing);
        assert_eq!(rig.palette.entry(1), fade_out_step(&part.picture_palette, 63).entry(1));
        assert_ne!(rig.palette.entry(1), part.picture_palette.entry(1));
        assert_eq!(HOLD_TICKS, 567);
    }

    #[test]
    fn the_gate_holds_for_five_thousand_ticks_when_the_cue_never_comes() {
        // `:107`'s bound is on iterations, the break's own tick among them.
        let (mut rig, mut part) = started();
        let last_hold = FADE_IN_FIRST_TICK + FADE_IN_STEPS + GATE_LIMIT - 1;
        run_ticks(&mut rig, &mut part, last_hold + 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {last_hold}: the last of the five thousand");
        part.frame(&mut rig.ctx(last_hold + 1));
        assert_eq!(rig.palette.entry(1), fade_out_step(&part.picture_palette, 63).entry(1));
    }

    #[test]
    fn the_gates_check_reads_the_previous_ticks_words() {
        // Every tick answers -15 (`zplus` 1, row 49, one above the threshold).
        // The first gate tick's check reads the fade-in's last tick's words,
        // so the gate breaks at once: the break's frame, the aligned frame,
        // then `c = 63`.
        let (mut rig, mut part) = started_with(shell_rig());
        let crossing = vec![(1, 49); (FADE_IN_FIRST_TICK + FADE_IN_STEPS + 2) as usize];
        rig.music = Some(scripted(crossing));

        let first_gate_tick = FADE_IN_FIRST_TICK + FADE_IN_STEPS;
        assert_eq!(FIRST_INDEX + first_gate_tick, HOLD_FIRST_INDEX);
        run_ticks(&mut rig, &mut part, first_gate_tick + 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {first_gate_tick} is the break's own frame");
        assert_eq!(part.phase, Phase::Gate { count: 0, held: 1 });
        run_ticks(&mut rig, &mut part, 1);
        assert_eq!(dac_entries(&rig), settled(&part), "tick {} is the aligned frame", first_gate_tick + 1);
        part.frame(&mut rig.ctx(first_gate_tick + 2));
        assert_eq!(rig.palette.entry(1), fade_out_step(&part.picture_palette, 63).entry(1));
    }

    #[test]
    fn a_run_with_no_player_is_refused_by_name() {
        let mut rig = shell_rig();
        let mut part = Endlogo::new();
        let error = part.init(&mut rig.ctx_without_music(0)).unwrap_err();
        assert_eq!(error.part, NAME);
        assert!(error.message.contains("107-111"), "{}", error.message);
        assert!(error.message.contains("no music"), "{}", error.message);
    }

    #[test]
    fn the_part_retires_after_the_fade_out_has_finished() {
        let (mut rig, mut part) = started_with(shell_rig().with(trace_second5()));
        let last = PART_LAST_INDEX - FIRST_INDEX;
        run_ticks(&mut rig, &mut part, last);
        assert_eq!(part.frame(&mut rig.ctx(last)), PartStatus::Continuing, "the last step draws");
        assert_eq!(rig.palette.entry(0), [0, 0, 0], "c = 0 is black");
        assert_eq!(part.frame(&mut rig.ctx(last + 1)), PartStatus::Finished);
    }

    // --- the derivation, run over the reference's own frames ----------------

    /// The DAC after fade-in step `c`: the white, then steps `0..=c` through
    /// `setpalarea` with `count` entries (255 here, 254 is BEGLOGO's).
    fn fade_in_dac(picture: &Palette, c: u32, count: u32) -> Palette {
        let mut dac = white();
        for step in 0..=c {
            setpalarea(&mut dac, &white_to_picture(picture, step), 0, count);
        }
        dac
    }

    /// The DAC after fade-out step `c` and every step before it, from `c = 63`.
    fn dac_after_fade_out(picture: &Palette, c: u32) -> Palette {
        let mut dac = white();
        for step in (c..=FADE_OUT_STEPS - 1).rev() {
            setpalarea(&mut dac, &fade_out_step(picture, step), 0, SETPALAREA_ENTRIES);
        }
        dac
    }

    /// The DAC `:90-91` leaves, entry 255 unwritten.
    fn white() -> Palette {
        let mut dac = Palette::new();
        for entry in 0..=254u8 {
            dac.set_entry(entry, [63, 63, 63]);
        }
        dac
    }

    /// Each fade keyframe is run against its ramp's whole range of `c`: one
    /// `c` reproduces its 320×400 RGB exactly, which names the index its step
    /// is displayed on (`endlogo.md` §3).
    #[test]
    fn the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit() {
        let image = load_image(NAME, PICTURE_PATH).unwrap();

        // The decode is the reference's held frame, byte for byte.
        assert!(image.pixels() == &reference("endlogo", 900, "idx")[..], "the decode is not 900's indices");

        // The picture indexes 0-71, inside the 170 entries the 510-byte upload
        // writes: the upload's byte count cannot show on a keyframe.
        let used: std::collections::BTreeSet<u8> = image.pixels().iter().copied().collect();
        assert_eq!(used.len(), 72, "the picture's distinct indices");
        assert_eq!(used.iter().next_back(), Some(&71));

        let early = reference("endlogo", 500, "rgb");
        let mut at_500 = Vec::new();
        let mut beglogo_upload = Vec::new();
        for c in 0..=128u32 {
            if rgb_of(image.pixels(), &fade_in_dac(&image.palette, c, SETPALAREA_ENTRIES)) == early {
                at_500.push(c);
            }
            if rgb_of(image.pixels(), &fade_in_dac(&image.palette, c, 254)) == early {
                beglogo_upload.push(c);
            }
        }
        assert_eq!(at_500, [49], "500 is the fade-in's ramp at c = 49, uniquely");
        assert_eq!(500 - at_500[0], FADE_IN_FIRST_INDEX);
        assert_eq!(beglogo_upload, at_500, "the upload's byte count is invisible on this art");

        // The fade-out counts down: 1180 shows `63 - (1180 - first)`.
        let late = reference("endlogo", 1180, "rgb");
        let at_1180: Vec<u32> = (0..=63u32)
            .rev()
            .filter(|&c| rgb_of(image.pixels(), &dac_after_fade_out(&image.palette, c)) == late)
            .collect();
        assert_eq!(at_1180, [30], "1180 is the fade-out's ramp at c = 30, uniquely");
        assert_eq!(1180 - (FADE_OUT_STEPS - 1 - at_1180[0]), FADE_OUT_FIRST_INDEX);

        // 900 is past the fade-in's convergence: it bounds the hold only.
        let held = reference("endlogo", 900, "rgb");
        let at_900: Vec<u32> = (0..=128u32)
            .filter(|&c| rgb_of(image.pixels(), &fade_in_dac(&image.palette, c, SETPALAREA_ENTRIES)) == held)
            .collect();
        assert_eq!(at_900, [126, 127, 128], "900 is converged, and only bounds the hold");

        const { assert!(PART_LAST_INDEX > 1200 && PART_LAST_INDEX <= 1240) };
    }
}
