//! BEGLOGO — the Future Crew logo, white, fading in over a held picture.
//!
//! `BEG/BEG.C` (87 lines) is the part itself, over `BEG/ASM.ASM` (`lineblit`,
//! `setpalarea`) and `BEG/READP.C` (the Origin RLE row reader), and it is four
//! acts over a screen that never moves:
//!
//! 1. `:18-21` — chain-4 off, and 64 KB of video memory filled with index 15.
//! 2. `:24` — 32 vertical blanks of white.
//! 3. `:25-78` — the DAC (`:25-34`), the CRTC and the AC (`:40-71`), and the
//!    400-row RLE decode and plane blit (`:73-78`).
//! 4. `:80-86` — 129 steps of a white→picture palette ramp, one per vertical
//!    blank, and then the picture's own palette once more.
//!
//! The shipped build is `BEG/`, not `ENDPIC/`: the reference's held 900 is
//! `BEG/SRTITLE.UP` under its own palette, so the picture is `SRTITLE.UP` and
//! there is **no music wait** (`ENDPIC/BEG.C:48`'s `dis_muscode(0xf0)` is the
//! build that did not ship; `docs/fidelity/beglogo.md` §1).
//!
//! Only the palette moves after the blit, so the picture is blitted once. The
//! blit is not in `init`: `:24`'s 32 vertical blanks come before `:73-78`, and
//! the reference's 430 (all index 15) is inside them. Nine frames before the
//! fade are not modelled (A5, `FADE_FIRST_INDEX`).
//!
//! The picture, the 400-line view, the blit, the white-to-picture ramp and
//! `setpalarea` are [`logo`](super::logo)'s, shared with ENDLOGO.

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::Palette;

use crate::parts::logo::{blit, load_picture, set_400_line_view, setpalarea, white_to_picture};

/// The part's name: what `--part` accepts and what the registry lists.
const NAME: &str = "beglogo";

/// The picture, read in its original format at runtime: `readp(palette,-1,pic)`
/// copies its palette out of the container and `:74-78` decodes 400 rows.
const PICTURE_PATH: &str = "assets/BEG/SRTITLE.UP";

/// The index `:20-21` fills video memory with, and so the whole screen at the
/// reference's 430.
const WHITE_FIELD: u8 = 15;

/// The overscan colour `:66-68` leaves in AC 0x11 — an entry number, not a
/// colour — and `:32-34` is what makes entry 255 black.
const OVERSCAN_ENTRY: u8 = 255;

/// The reference index a run that starts at this part draws its first frame at:
/// the demo's first graphics frame, which is also `shell::boot_anchor`'s entry
/// for `beglogo`. `PartCtx::frame` counts this part's ticks from there, so the
/// reference index any tick draws at is this plus `ctx.frame`.
const FIRST_INDEX: u32 = crate::shell::FIRST_GRAPHICS_FRAME;

/// The reference index the fade's step `c = 0` is displayed on, measured: 470
/// is the ramp at c = 24 and 560 at c = 114, uniquely (`beglogo.md` §3).
///
/// The source accounts for 33 of the 42 frames from [`FIRST_INDEX`] (404): the
/// 32 vertical blanks of `BEG/BEG.C:24` and the tick `:25-78` runs on. The other
/// nine are not modelled and their cause is unidentified (A5); the port holds
/// the white field through them, as the reference does.
const FADE_FIRST_INDEX: u32 = 446;

/// The part tick the fade's step `c = 0` is displayed on.
const FADE_FIRST_TICK: u32 = FADE_FIRST_INDEX - FIRST_INDEX;

/// `BEG/BEG.C:24`'s `for(a=0;a<32;a++) dis_waitb()`: one vertical blank per
/// tick, so the white field holds for the part's first 32 ticks and `:25-78`
/// runs on the tick that follows them.
const LEAD_IN_TICKS: u32 = 32;

/// `BEG/BEG.C:80`'s `for(c=0;c<=128;c++)`: 129 fade steps, one per tick from
/// [`FADE_FIRST_TICK`]. The tick after the last one is `:86`'s final write.
const FADE_STEPS: u32 = 129;

/// The picture and the palette it indexes, both as `readp` leaves them.
pub struct Beglogo {
    /// The decoded picture, row-major at 320 pixels — the shape
    /// `readp(rowbuf,y,pic)` decodes one row of. Empty until `init`.
    picture: Vec<u8>,
    /// The container's palette, six bits a channel: the source of the fade's
    /// ramp and, at `:86`, the screen's own colours. `BEG/BEG.C` calls it
    /// `palette[]`; it is not the live DAC.
    picture_palette: Palette,
}

impl Beglogo {
    /// A part with no picture: `init` is what reads one.
    pub fn new() -> Beglogo {
        Beglogo {
            picture: Vec::new(),
            picture_palette: Palette::new(),
        }
    }
}

impl Part for Beglogo {
    fn name(&self) -> &'static str {
        NAME
    }

    /// `BEG/BEG.C:18-78`: chain-4 off, the white field, the DAC, the CRTC and
    /// the AC, and the picture decoded — everything but the blit, which belongs
    /// to [`LEAD_IN_TICKS`].
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // `:18-21`: chain-4 off and 64 KB of index 15; on the chunky surface,
        // one `clear`.
        ctx.surface.clear(WHITE_FIELD);
        // `:42-71`: display start 0, the 400-line screen, AC 0x11 = 255.
        set_400_line_view(ctx, OVERSCAN_ENTRY);
        // `:73-78`.
        (self.picture, self.picture_palette) = load_picture(NAME, PICTURE_PATH)?;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match ctx.frame {
            // `:24` — the 32 vertical blanks. The white field holds.
            tick if tick < LEAD_IN_TICKS => PartStatus::Continuing,
            // `:25-78` — the DAC, and then the picture into video memory.
            LEAD_IN_TICKS => {
                ctx.set_palette(white_dac());
                blit(&self.picture, ctx);
                PartStatus::Continuing
            }
            // The nine unmodelled ticks ([`FADE_FIRST_INDEX`], A5): white.
            tick if tick < FADE_FIRST_TICK => PartStatus::Continuing,
            tick => {
                let step = tick - FADE_FIRST_TICK;
                if step < FADE_STEPS {
                    // `:82-84` — one step of the ramp per vertical blank.
                    let ramp = white_to_picture(&self.picture_palette, step);
                    setpalarea(ctx.palette, &ramp, 0, 254);
                    PartStatus::Continuing
                } else {
                    // `:86`, the picture's own palette once more; `:87` returns.
                    setpalarea(ctx.palette, &self.picture_palette, 0, 254);
                    PartStatus::Finished
                }
            }
        }
    }
}

/// `BEG/BEG.C:25-34`: entries 0-254 white, then entry 255 (the overscan colour
/// AC 0x11 names) black.
fn white_dac() -> Palette {
    let mut dac = Palette::new();
    for entry in 0..255u8 {
        dac.set_entry(entry, [63, 63, 63]);
    }
    dac.set_entry(255, [0, 0, 0]);
    dac
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::load_image;
    use crate::parts::logo::{PICTURE_HEIGHT, PICTURE_WIDTH};
    use crate::parts::test_rig::{reference, rgb_of, Rig};

    use crate::shell::{VIEW_HEIGHT, VIEW_WIDTH};

    /// An index no path through the part writes, used as a marker for "the part
    /// left the DAC the shell installed alone".
    const UNWRITTEN: u8 = 200;

    /// The shell's 320×200 view and a DAC whose entry 15 is white, as the mode
    /// set leaves it.
    fn shell_rig() -> Rig {
        Rig::new(VIEW_WIDTH, VIEW_HEIGHT).palette_entry(15, [63, 63, 63]).palette_entry(UNWRITTEN, [1, 2, 3])
    }

    /// A part whose `init` has run, and the rig it ran against.
    fn started() -> (Rig, Beglogo) {
        let mut rig = shell_rig();
        let mut part = Beglogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        (rig, part)
    }

    #[test]
    fn the_picture_is_read_at_its_own_row_stride_whatever_the_part_before_left() {
        // PAM leaves `Some(320)`; on the 640-wide surface that would read every
        // second scanline from the picture's margin (`beglogo.md` §5).
        let mut rig = shell_rig();
        rig.view.y_line_offset = Some(320);
        let mut part = Beglogo::new();
        part.init(&mut rig.ctx(0)).unwrap();
        assert_eq!(rig.view.y_line_offset, None);
    }

    /// Runs the part for `ticks` ticks, numbered from zero.
    fn run_ticks(rig: &mut Rig, part: &mut Beglogo, ticks: u32) {
        for at in 0..ticks {
            part.frame(&mut rig.ctx(at));
        }
    }

    #[test]
    fn the_opening_white_is_index_15_over_the_palette_the_shell_left() {
        // `:25-34` writes the DAC only after the 32 waits, so 430 is index 15
        // through the inherited palette: the part must not clear the DAC.
        let (mut rig, mut part) = started();
        assert_eq!(rig.surface.get(0, 0), Some(15));
        assert_eq!(rig.surface.get(319, 399), Some(15));
        assert_eq!(rig.palette.entry(15), [63, 63, 63]);
        run_ticks(&mut rig, &mut part, 430 - FIRST_INDEX);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 15));
        assert_eq!(rig.palette.entry(UNWRITTEN), [1, 2, 3]);
    }

    #[test]
    fn the_fades_white_is_written_into_entries_0_to_254_and_255_is_black() {
        // No reference frame shows entry 255; it is GLENZ that needs it black
        // (`zoomer2`'s bands, `reference-indices.md` §3.5).
        let (mut rig, mut part) = started();
        assert_ne!(rig.palette.entry(0), [63, 63, 63], "the shell's entry 0 is not white");
        run_ticks(&mut rig, &mut part, LEAD_IN_TICKS + 1);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(255), [0, 0, 0], "`:32-34`, and the overscan colour");
        assert_eq!(rig.palette.entry(UNWRITTEN), [63, 63, 63]);
    }

    #[test]
    fn the_picture_is_blitted_before_the_fade_starts() {
        let (mut rig, mut part) = started();
        run_ticks(&mut rig, &mut part, LEAD_IN_TICKS);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 15));
        part.frame(&mut rig.ctx(LEAD_IN_TICKS));
        assert_eq!(&rig.surface.row(0)[..PICTURE_WIDTH], &part.picture[..PICTURE_WIDTH]);
        assert_eq!(
            &rig.surface.row(PICTURE_HEIGHT - 1)[..PICTURE_WIDTH],
            &part.picture[(PICTURE_HEIGHT - 1) * PICTURE_WIDTH..]
        );
        // c = 0 is still white.
        run_ticks(&mut rig, &mut part, FADE_FIRST_TICK);
        part.frame(&mut rig.ctx(FADE_FIRST_TICK));
        for index in 0..=254u8 {
            assert_eq!(rig.palette.entry(index), [63, 63, 63], "entry {index} at c = 0");
        }
        assert_eq!(rig.palette.entry(255), [0, 0, 0]);
    }

    /// The DAC after fade step `c`: `:25-34`'s white, then steps `0..=c` of the
    /// ramp through the shipped `setpalarea`, or with `fixed` a full 762-byte
    /// upload.
    fn scan_dac(picture: &Palette, c: u32, fixed: bool) -> Palette {
        let mut dac = white_dac();
        for step in 0..=c {
            let ramp = white_to_picture(picture, step);
            if fixed {
                for entry in 0..=254u8 {
                    dac.set_entry(entry, ramp.entry(entry));
                }
            } else {
                setpalarea(&mut dac, &ramp, 0, 254);
            }
        }
        dac
    }

    /// Every `c` in 0…128 rendered and compared with the reference bit for bit:
    /// the scan `beglogo.md` §3 records.
    #[test]
    fn the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit() {
        let image = load_image(NAME, PICTURE_PATH).unwrap();
        let (early, late, finished) = (reference("beglogo", 470, "rgb"), reference("beglogo", 560, "rgb"), reference("beglogo", 900, "rgb"));
        let mut at_470 = Vec::new();
        let mut at_560 = Vec::new();
        let mut at_900 = Vec::new();
        let mut fixed = [Vec::new(), Vec::new(), Vec::new()];
        for c in 0..=128u32 {
            let dac = scan_dac(&image.palette, c, false);
            let frame = rgb_of(image.pixels(), &dac);
            if frame == early { at_470.push(c); }
            if frame == late { at_560.push(c); }
            if frame == finished { at_900.push(c); }
            let frame = rgb_of(image.pixels(), &scan_dac(&image.palette, c, true));
            if frame == early { fixed[0].push(c); }
            if frame == late { fixed[1].push(c); }
            if frame == finished { fixed[2].push(c); }
        }

        assert_eq!(at_470, [24], "470 is the ramp at c = 24, uniquely");
        assert_eq!(at_560, [114], "560 is the ramp at c = 114, uniquely");
        assert_eq!(470 - at_470[0], FADE_FIRST_INDEX);
        assert_eq!(560 - at_560[0], FADE_FIRST_INDEX);
        // 900 is past the ramp's convergence: it pins the end, not the start.
        assert_eq!(at_900, [126, 127, 128]);
        // The upload's byte-count bug is invisible on this art (`bugs.md` §4).
        assert_eq!(fixed, [at_470.clone(), at_560.clone(), at_900.clone()]);
    }
}
