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
//! **Which of the tree's two BEGLOGO builds shipped is settled, and it is
//! `BEG/`.** The reference's frame 900 is past the end of the fade, so the DAC
//! it displays *is* the picture's palette, and the probe `reference/parts/
//! beglogo/manifest.json` records for it matches `BEG/SRTITLE.UP`'s palette
//! entry for entry (decoding it and widening reproduces `frame_000900.idx`
//! with 0 of 128,000 pixels different) and none of `ENDPIC/`'s. Two
//! consequences: the picture is `SRTITLE.UP` here, and **there is no music
//! wait** — the `dis_muscode(0xf0)` line is `ENDPIC/BEG.C:48`, in the build
//! that did not ship, while `BEG/BEG.C` calls `dis_partstart`, `dis_waitb` and
//! nothing else. `docs/fidelity/beglogo.md` carries the derivation.
//!
//! Only the palette moves during those 129 steps, so the picture is decoded and
//! blitted **once** and every later tick writes a palette. The blit is not part
//! of `init`: `:24`'s 32 vertical blanks come *before* `:73-78`, and the
//! reference's frame 430 — 128,000 bytes of index 15, no picture in video
//! memory yet — is inside them.
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

/// The reference index the fade's step `c = 0` is displayed on.
///
/// Derived in `docs/fidelity/beglogo.md` by matching each fade keyframe's RGB
/// bit for bit: 470 is the ramp at c = 24 and 560 at c = 114, uniquely, which
/// puts c = 0 at 446.
///
/// A run that starts here begins drawing at [`FIRST_INDEX`] (404) and the
/// source accounts for 33 of the 42 frames between there and 446: the 32
/// vertical blanks of `BEG/BEG.C:24` and the tick `:25-78` runs on. **The other
/// nine are frames the port does not model, and their cause is unidentified** —
/// `docs/fidelity/APPROXIMATIONS.md` A5 records what is ruled out and what a
/// later phase should watch. The port draws the white field throughout, which
/// is what the reference shows at every index inside the window it holds.
///
/// 446 itself is measured, not derived from that split: it is where the ramp
/// is *displayed*, and 470 and 560 answer it uniquely
/// (`docs/fidelity/beglogo.md` §3).
const FADE_FIRST_INDEX: u32 = 446;

/// The part tick the fade's step `c = 0` is displayed on: `FADE_FIRST_INDEX`
/// less this part's own first index. 33 of its 42 ticks are `:24`'s 32 vertical
/// blanks and the tick `:25-78` runs on; the other nine are the frames the port
/// does not model ([`FADE_FIRST_INDEX`], A5), which the port holds the white
/// field through instead of re-paying.
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
        // `:18-19` sets the sequencer's map mask to 0x0f — chain-4 off — and
        // `:20-21` memsets 64 KB of A-space with 15. The port's surface is
        // chunky, so that is one `clear`, and it covers the whole plane the
        // original filled: video memory is 64 KB a plane either way.
        ctx.surface.clear(WHITE_FIELD);
        // `:42-71`: display start 0, the 400-line screen, and AC 0x11 = 255,
        // the overscan colour, which `:32-34` makes black. In a run that
        // starts here the registers `BEG.C` leaves alone are the shell's: no
        // split, no pan.
        set_400_line_view(ctx, OVERSCAN_ENTRY);

        // `:73-78`. `docs/fidelity/beglogo.md` records that the decode is
        // byte-identical to the reference's held frame, 0 of 128,000 pixels
        // apart.
        (self.picture, self.picture_palette) = load_picture(NAME, PICTURE_PATH)?;

        // `:17`'s `dis_partstart()` needs no modelling of its own: it is a
        // *version check* on the resident demo server — `DIS/DISC.ASM:51-64`
        // calls `_dis_version`, prints `ERROR: DIS not loaded.` and exits 3 if
        // it is absent, and returns immediately otherwise. It costs no frame,
        // so nothing of it belongs in `FADE_FIRST_TICK`.
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        match ctx.frame {
            // `:24` — the 32 vertical blanks. The white field holds.
            tick if tick < LEAD_IN_TICKS => PartStatus::Continuing,
            // `:25-78` — the DAC, and then the picture into video memory.
            LEAD_IN_TICKS => {
                install_white_dac(ctx);
                blit(&self.picture, ctx);
                PartStatus::Continuing
            }
            // The nine ticks between `:78` and the fade's first vertical blank:
            // frames the port does not model ([`FADE_FIRST_INDEX`],
            // `docs/fidelity/APPROXIMATIONS.md` A5). The screen is white
            // through them either way, and not only because the DAC is:
            // `:25-31` left every entry the picture indexes white.
            tick if tick < FADE_FIRST_TICK => PartStatus::Continuing,
            tick => {
                let step = tick - FADE_FIRST_TICK;
                if step < FADE_STEPS {
                    // `:82-84` — one step of the ramp per vertical blank.
                    let ramp = white_to_picture(&self.picture_palette, step);
                    setpalarea(ctx.palette, &ramp, 0, 254);
                    PartStatus::Continuing
                } else {
                    // `:86` — the picture's own palette once more — and `:87`
                    // closes `main()`. Everything the screen does afterwards
                    // belongs to the next part (`reference-indices.md` §3.5).
                    setpalarea(ctx.palette, &self.picture_palette, 0, 254);
                    PartStatus::Finished
                }
            }
        }
    }
}

/// `BEG/BEG.C:25-34`: `outp(0x3c8,0)` and 255 triples of 63, then one of 0.
///
/// Entry 255 is the one the loop does not reach — `for(a=0;a<255;a++)` writes
/// entries 0-254 — and the one `:32-34` writes black. It is also the overscan
/// colour `:66-68` leaves in AC 0x11, so this is the write that makes the
/// part's border black.
fn white_dac() -> Palette {
    let mut dac = Palette::new();
    for entry in 0..255u8 {
        dac.set_entry(entry, [63, 63, 63]);
    }
    dac.set_entry(255, [0, 0, 0]);
    dac
}

/// [`white_dac`] into the live palette — what `:25-34` does to the hardware.
fn install_white_dac(ctx: &mut PartCtx) {
    ctx.set_palette(white_dac());
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

    /// The screen and the DAC the shell hands a run's first part: a 320×200
    /// view, and `hardware_default_palette`, whose entry 15 is white — which
    /// is what makes the reference's 430 white before the part has written a
    /// single DAC entry. No player: none of these tests asks for a cue.
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
        // Mid-chain the view arrives with the predecessor's line offset: PAM (`pam.rs`, `Some(LINE_PIXELS)`) leaves it
        // at 320 pixels a row. In the original that is the 320-pixel tweak's CRTC 0x13,
        // the stride this part's own `lineblit(vram+y*80U, ...)` draws at, so the
        // picture is read row by row. On the port's 640-wide surface a kept `Some(320)`
        // would read every second scanline from the picture's right margin — the full
        // run's measurement (`beglogo.md` §5) — so the part reads its own
        // rows, the unset offset.
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
        // BEG/BEG.C:20-21 fills A-space with index 15, and `:25-34` writes the
        // DAC only after the 32 waits — so the screen the reference captures at
        // 430 is index 15 through the *inherited* palette, where entry 15 is
        // (63,63,63). The part must not clear the DAC at start.
        let (mut rig, mut part) = started();
        assert_eq!(rig.surface.get(0, 0), Some(15));
        assert_eq!(rig.surface.get(319, 399), Some(15));
        assert_eq!(rig.palette.entry(15), [63, 63, 63]);
        // Frame 430 is the tick whose `ctx.frame` is 430 - FIRST_INDEX, inside
        // the lead-in, so the whole white field is drawn through the DAC the
        // shell installed — marker entry and all.
        run_ticks(&mut rig, &mut part, 430 - FIRST_INDEX);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 15));
        assert_eq!(rig.palette.entry(UNWRITTEN), [1, 2, 3]);
    }

    #[test]
    fn the_fades_white_is_written_into_entries_0_to_254_and_255_is_black() {
        // `:25-34`: 255 iterations of (63,63,63) then one of (0,0,0). **No
        // reference frame reaches entry 255**: the picture indexes 1-79, the
        // white field is 15, the ramp's upload stops inside entry 169, and the
        // frame the overscan would show it in carries no border — the dump sets
        // `border: false` (`headless.rs`). This write is ported because the
        // source performs it, and what it is for is the *next* part: entry 255
        // black is why GLENZ's `zoomer2` bands are black over a greying picture
        // (`reference-indices.md` §3.5).
        let (mut rig, mut part) = started();
        assert_ne!(rig.palette.entry(0), [63, 63, 63], "the shell's entry 0 is not white");
        // The 32 blanks, then the tick `:25-34` runs on — the DAC is written
        // one tick after them and not in `init`, because the reference's 430
        // falls 26 ticks in and shows the shell's DAC, not this one.
        run_ticks(&mut rig, &mut part, LEAD_IN_TICKS + 1);
        assert_eq!(rig.palette.entry(0), [63, 63, 63]);
        assert_eq!(rig.palette.entry(254), [63, 63, 63]);
        assert_eq!(rig.palette.entry(255), [0, 0, 0], "`:32-34`, and the overscan colour");
        // 255 iterations, not 256: the white run starts at 0 and the black
        // write is the 256th entry of the DAC, not a 256th iteration.
        assert_eq!(rig.palette.entry(UNWRITTEN), [63, 63, 63]);
    }

    #[test]
    fn the_picture_is_blitted_before_the_fade_starts() {
        // The screen at the first fade step is still white: the blit lands in
        // surface memory but the DAC shows entry 15 everywhere until c moves.
        let (mut rig, mut part) = started();
        // `:24`'s 32 vertical blanks: video memory is still the white field,
        // which is what the reference's 430 holds (128,000 bytes of 15).
        run_ticks(&mut rig, &mut part, LEAD_IN_TICKS);
        assert!(rig.surface.pixels().iter().all(|pixel| *pixel == 15));
        // `:25-78` — and the picture is in video memory from here on.
        part.frame(&mut rig.ctx(LEAD_IN_TICKS));
        assert_eq!(&rig.surface.row(0)[..PICTURE_WIDTH], &part.picture[..PICTURE_WIDTH]);
        assert_eq!(
            &rig.surface.row(PICTURE_HEIGHT - 1)[..PICTURE_WIDTH],
            &part.picture[(PICTURE_HEIGHT - 1) * PICTURE_WIDTH..]
        );
        // The white DAC is up with it, and the first fade step is still white:
        // c = 0 writes 63 into every entry the ramp covers.
        run_ticks(&mut rig, &mut part, FADE_FIRST_TICK);
        part.frame(&mut rig.ctx(FADE_FIRST_TICK));
        for index in 0..=254u8 {
            assert_eq!(rig.palette.entry(index), [63, 63, 63], "entry {index} at c = 0");
        }
        assert_eq!(rig.palette.entry(255), [0, 0, 0]);
    }

    // --- the derivation, run over the reference's own frames ----------------

    /// The DAC as it stands after fade step `c`: `:25-34`'s white writes, then
    /// steps `0..=c` of the ramp through the shipped [`setpalarea`].
    ///
    /// The reference side of the scan below, and the reason it can be a scan:
    /// the whole DAC is rebuilt from the part's own arithmetic rather than
    /// adjusted step by step, so `c` is the only thing that varies.
    fn dac_after(picture: &Palette, c: u32) -> Palette {
        scan_dac(picture, c, false)
    }

    /// The same with the bug "fixed" — a full 762-byte upload of every step.
    fn dac_after_a_fixed_upload(picture: &Palette, c: u32) -> Palette {
        scan_dac(picture, c, true)
    }

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

    /// **The constant is a derivation, not a fit**.
    ///
    /// Every `c` in 0…128 is run through the source's own arithmetic and the
    /// whole 320×400 frame compared with the reference's, bit for bit: the two
    /// moving keyframes answer at exactly one `c` each, and both agree on the
    /// same first index. `docs/fidelity/beglogo.md` carries the table; this is
    /// the scan it records.
    #[test]
    fn the_fade_constants_are_derived_by_matching_the_reference_bit_for_bit() {
        let image = load_image(NAME, PICTURE_PATH).unwrap();

        // The decode is the strongest statement available that the decoder and
        // the art agree: the reference's held frame is its indices, byte for
        // byte — 0 of 128,000 pixels different.
        let held = reference("beglogo", 900, "idx");
        // The lengths first: `zip` stops at the shorter side, so a truncated
        // reference frame would compare a prefix and count zero differences.
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

        let (early, late, finished) = (reference("beglogo", 470, "rgb"), reference("beglogo", 560, "rgb"), reference("beglogo", 900, "rgb"));
        let mut at_470 = Vec::new();
        let mut at_560 = Vec::new();
        let mut at_900 = Vec::new();
        let mut fixed = [Vec::new(), Vec::new(), Vec::new()];
        for c in 0..=128u32 {
            let dac = dac_after(&image.palette, c);
            let frame = rgb_of(image.pixels(), &dac);
            if frame == early { at_470.push(c); }
            if frame == late { at_560.push(c); }
            if frame == finished { at_900.push(c); }
            let frame = rgb_of(image.pixels(), &dac_after_a_fixed_upload(&image.palette, c));
            if frame == early { fixed[0].push(c); }
            if frame == late { fixed[1].push(c); }
            if frame == finished { fixed[2].push(c); }
        }

        assert_eq!(at_470, [24], "470 is the ramp at c = 24, uniquely");
        assert_eq!(at_560, [114], "560 is the ramp at c = 114, uniquely");
        // Both keyframes are the same first index: 470 - 24 = 560 - 114.
        assert_eq!(470 - at_470[0], FADE_FIRST_INDEX);
        assert_eq!(560 - at_560[0], FADE_FIRST_INDEX);
        // 900 is past the ramp's convergence — c = 126, 127 and 128 are the
        // same palette — so it pins the end and not the start. Resolved by the
        // picture, not by arithmetic: the ramp has converged and only the two
        // above can say where it began.
        assert_eq!(at_900, [126, 127, 128]);
        // And the upload's byte-count bug is invisible on this art: a port
        // that wrote the full 762 bytes answers every one of these frames at
        // the same `c`. That is why `bugs.md` can only point at the part that
        // next paints into entries 170-254.
        assert_eq!(fixed, [at_470.clone(), at_560.clone(), at_900.clone()]);
    }
}
