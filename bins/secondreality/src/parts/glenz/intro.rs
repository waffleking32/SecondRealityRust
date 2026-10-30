//! GLENZ's intro: `zoomer2` (`ZOOMER.C:18-81`) and the FC floor bounce
//! (`MAIN.C:320-350`).
//!
//! Both effects draw into the part's **400-row** 320-pixel buffer while the
//! inherited BEGLOGO screen is still up, and both are one `dis_waitb` a tick:
//! the zoomer draws its bands and then waits, so a call here is the work of
//! one displayed frame. The zoomer's 47th call caps `zy` at 260; the 48th sees
//! it and breaks (the source's `if(zy==260) break` at the loop top), which is
//! why [`Zoomer::tick`] returns `false` rather than drawing a 48th band.
//!
//! The bounce is the source's integer spring (`MAIN.C:322-350`): `yy` grows
//! by `ya`, flips `ya` every time `yy > 48*16`, and stops when the flipped
//! speed is under 4. That is 177 drawn frames and a last band of
//! `y1 = 153, y2 = 200`; the test pins both. Rows above the band are **not**
//! cleared — the band's earlier, higher passes leave their picture there,
//! which is what makes the final buffer the reference's held 1499 screen.
//!
//! The source's `memset(vram + y*80, ...)` runs in Mode X's chain-4, so one
//! addressed byte is four plane bytes, i.e. four pixels; the port's buffer is
//! already linear 320-pixel rows (`docs/decisions.md` D36), so a band row is
//! 320 bytes here. The FC picture is 320x200: `fcrow` is its rows 0..99 and
//! `fcrow2` its rows 100..115 (`MAIN.C:306-313`).

use sr_engine::Palette;

/// Displayed width: the intro's 320-wide screen and the buffer's row.
pub(crate) const WIDTH: usize = 320;
/// The inherited Mode X buffer: 400 rows of 320 pixels.
pub(crate) const VRAM_PIXELS: usize = WIDTH * 400;
/// The 320x200 frame the bounce fills and `bgpic` snapshots.
pub(crate) const SCREEN_PIXELS: usize = WIDTH * 200;
/// The 47th zoomer call's terminal `zy` (`ZOOMER.C:44-45`).
const ZOOMER_MAX: i32 = 260;
/// The zoomer's palette fade target and cap (`ZOOMER.C:59-65`).
const FADE_LEVEL: i32 = 30;
const FADE_CAP: i32 = 32;
/// The bounce's spring limit: `48*16` (`MAIN.C:325`).
const BOUNCE_LIMIT: i32 = 48 * 16;

/// `zoomer2`'s loop state (`ZOOMER.C:18-81`).
pub(crate) struct Zoomer {
    /// `pal1`: the live DAC, read once before the loop (`:32-33`).
    pal1: [[u8; 3]; 256],
    /// `pal2`: the faded copy uploaded to entries 0..127 each tick.
    pal2: [[u8; 3]; 128],
    /// The loop's own `frame`, from 0 (`:22`, `:37`).
    frame: i32,
    zly: i32,
    zy: i32,
    zya: i32,
    zly2: i32,
    zy2: i32,
}

impl Zoomer {
    /// The source's prologue: read the DAC into `pal1` and zero the bands.
    pub(crate) fn new(palette: &Palette) -> Zoomer {
        let mut pal1 = [[0u8; 3]; 256];
        for (index, entry) in pal1.iter_mut().enumerate() {
            *entry = palette.entry(index as u8);
        }
        Zoomer {
            pal1,
            pal2: [[0u8; 3]; 128],
            frame: 0,
            zly: 0,
            zy: 0,
            zya: 0,
            zly2: 0,
            zy2: 0,
        }
    }

    /// One loop body (`ZOOMER.C:38-69`).
    ///
    /// Returns `false` on the iteration that finds `zy == 260` at the top and
    /// breaks without drawing; the caller then runs the source's epilogue
    /// ([`finish`]). A `true` return drew the bands, built `pal2`, uploaded
    /// entries 0..127 and counted a frame.
    pub(crate) fn tick(&mut self, vram: &mut [u8], palette: &mut Palette) -> bool {
        if self.zy == ZOOMER_MAX {
            return false;
        }
        // `zly=zy; zya++; zy+=zya/4; if(zy>260) zy=260;` (`:41-44`).
        self.zly = self.zy;
        self.zya += 1;
        self.zy += self.zya / 4;
        if self.zy > ZOOMER_MAX {
            self.zy = ZOOMER_MAX;
        }
        // `v=vram+zly*80; for(y=zly;y<=zy;y++) memset(v,255,80)` (`:45-50`):
        // the top band grows downward, one 320-pixel row a line.
        for row in self.zly..=self.zy {
            let at = row as usize * WIDTH;
            vram[at..at + WIDTH].fill(255);
        }
        // `zly2=zy2; zy2=125*zy/260; v=vram+(399-zy2)*80;` (`:51-58`): the
        // bottom band starts at row `399 - zy2` and grows *down*, so its last
        // row is `399 - zly2`.
        self.zly2 = self.zy2;
        self.zy2 = 125 * self.zy / 260;
        let first = (399 - self.zy2) as usize;
        let last = (399 - self.zly2) as usize;
        for row in first..=last {
            let at = row * WIDTH;
            vram[at..at + WIDTH].fill(255);
        }
        // `c=frame; if(c>32)c=32; b=32-c; pal2[a]=(pal1[a]*b+30*c)>>5`
        // (`:59-65`), then `setpalarea(pal2,0,128)` after the wait (`:68`).
        let c = self.frame.min(FADE_CAP);
        let b = FADE_CAP - c;
        for (out, src) in self.pal2.iter_mut().zip(self.pal1.iter()) {
            for (out_channel, &src_channel) in out.iter_mut().zip(src.iter()) {
                *out_channel = ((i32::from(src_channel) * b + FADE_LEVEL * c) >> 5) as u8;
            }
        }
        self.frame += 1;
        for (index, entry) in self.pal2.iter().enumerate() {
            palette.set_entry(index as u8, *entry);
        }
        true
    }

    /// `zy` after the last drawn call: 260, the cap (`ZOOMER.C:44`). The
    /// test's readout of the 47-tick recurrence.
    #[cfg(test)]
    pub(crate) fn zy(&self) -> i32 {
        self.zy
    }

    /// The number of drawn frames (`ZOOMER.C:66`'s `frame`).
    #[cfg(test)]
    pub(crate) fn frames(&self) -> i32 {
        self.frame
    }
}

/// `zoomer2`'s epilogue (`ZOOMER.C:70-80`): entry 0 black, then every one of
/// the 400 rows cleared.
///
/// The source's clear condition is `y<=274 || y>=260`, which is true for every
/// `y` in 0..399; the band boundaries are not a gap. Entry 0 is set with three
/// `outp`, and the rest of the DAC is left alone.
pub(crate) fn finish(vram: &mut [u8], palette: &mut Palette) {
    palette.set_entry(0, [0, 0, 0]);
    vram.fill(0);
}

/// The FC bounce's loop state (`MAIN.C:322-350`).
pub(crate) struct Bounce {
    yy: i32,
    ya: i32,
    /// How many bodies have drawn; the source's loop has no counter.
    drawn: u32,
    y1: i32,
    y2: i32,
}

impl Bounce {
    /// `yy=0; ya=0` (`MAIN.C:320`).
    pub(crate) fn new() -> Bounce {
        Bounce {
            yy: 0,
            ya: 0,
            drawn: 0,
            y1: 0,
            y2: 0,
        }
    }

    /// One loop body (`MAIN.C:323-350`).
    ///
    /// Returns `false` when the spring's top-of-loop test breaks without
    /// drawing; a `true` return drew the four black rows above the band, the
    /// scaled `fcrow` rows, and `fcrow2`'s sixteen rows (8..15 black).
    pub(crate) fn tick(&mut self, vram: &mut [u8], fc: &[u8]) -> bool {
        // `ya++; yy+=ya;` (`:323-324`).
        self.ya += 1;
        self.yy += self.ya;
        // `if(yy>48*16) { yy-=ya; ya=-ya*2/3; if(ya>-4 && ya<4) break; }`
        // (`:325-330`). The C division truncates toward zero; `ya` is
        // positive here, so `-ya*2/3` is a negative truncating divide.
        if self.yy > BOUNCE_LIMIT {
            self.yy -= self.ya;
            self.ya = -self.ya * 2 / 3;
            if self.ya > -4 && self.ya < 4 {
                return false;
            }
        }
        // `y=yy/16; y1=130+y/2; y2=130+y*3/2;` (`:331-334`).
        let y = self.yy / 16;
        let y1 = 130 + y / 2;
        let y2 = 130 + y * 3 / 2;
        // `if(y2!=y1) b=25600/(y2-y1);` (`:335-336`). When `y2 == y1` the
        // second loop is empty and `b` is never read, as in the source.
        let mut b = 0;
        if y2 != y1 {
            b = 25600 / (y2 - y1);
        }

        // The four black rows above the band (`:337-343`).
        for row in (y1 - 4)..y1 {
            if row <= 199 {
                let at = row as usize * WIDTH;
                vram[at..at + WIDTH].fill(0);
            }
        }
        // The scaled picture rows (`:344-348`): `c` advances by `b` after
        // every row the loop visits, including the rows past 199 that the
        // `continue` skips, and `fcrow[c/256]` is row `c/256` of the picture.
        let mut c = 0;
        for row in y1..y2 {
            if row <= 199 {
                let source = (c / 256) as usize * WIDTH;
                let at = row as usize * WIDTH;
                vram[at..at + WIDTH].copy_from_slice(&fc[source..source + WIDTH]);
            }
            c += b;
        }
        // `fcrow2`'s sixteen rows: 0..7 from picture rows 100..107, 8..15
        // black (`:349-350`, `:309-313`).
        for (index, row) in (y2..y2 + 16).enumerate() {
            if row <= 199 {
                let at = row as usize * WIDTH;
                if index > 7 {
                    vram[at..at + WIDTH].fill(0);
                } else {
                    let source = (100 + index) * WIDTH;
                    vram[at..at + WIDTH].copy_from_slice(&fc[source..source + WIDTH]);
                }
            }
        }
        self.drawn += 1;
        self.y1 = y1;
        self.y2 = y2;
        true
    }

    /// The number of drawn frames: 177 for the shipped recurrence.
    #[cfg(test)]
    pub(crate) fn drawn(&self) -> u32 {
        self.drawn
    }

    /// The last drawn band's top and bottom rows (`y1`, `y2`).
    #[cfg(test)]
    pub(crate) fn band(&self) -> (i32, i32) {
        (self.y1, self.y2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::test_rig::reference;
    use sr_assets::Image;
    use std::path::Path;

    /// A palette whose entry `i` is `(i, 63-i, i/4)`, so `pal1`'s copy and
    /// the fade arithmetic are readable.
    fn test_palette() -> Palette {
        let mut palette = Palette::new();
        for index in 0..=255u8 {
            let index = i16::from(index);
            palette.set_entry(
                index as u8,
                [index as u8, (63 - index) as u8, (index / 4) as u8],
            );
        }
        palette
    }

    /// `FC.UH`'s pixels, decoded by the tree's own reader.
    fn fc_picture() -> Image {
        sr_assets::load_path(Path::new("../../assets/GLENZ/FC.UH")).unwrap()
    }

    /// The reference's held opening: the indexed plane the bounce's last frame
    /// must reproduce.
    fn held_reference() -> Vec<u8> {
        reference("glenz", 1499, "idx")
    }

    /// The 47 drawn ticks and the break on the 48th (`ZOOMER.C:38-44`), with
    /// the band the last draw fills.
    #[test]
    fn the_zoomer_draws_47_ticks_and_breaks_on_the_48th() {
        let mut palette = test_palette();
        let mut vram = vec![0u8; VRAM_PIXELS];
        let mut zoomer = Zoomer::new(&palette);
        for tick in 0..47 {
            assert!(zoomer.tick(&mut vram, &mut palette), "tick {tick} drew");
        }
        assert_eq!(zoomer.zy(), 260, "the 47th draw caps zy");
        assert_eq!(zoomer.frames(), 47);
        assert!(!zoomer.tick(&mut vram, &mut palette), "the 48th breaks");
        assert_eq!(zoomer.frames(), 47, "the break draws nothing");

        // The bands accumulate. The top band is 0..=260 (zly = zy(46) = 253,
        // zy = 260 on the last draw); the bottom bands tile from row 399 up
        // to 399 - zly2(47) = 278, the last draw covering 274..=278.
        let mut zoomer = Zoomer::new(&test_palette());
        let mut palette = test_palette();
        let mut vram = vec![0u8; VRAM_PIXELS];
        for _ in 0..47 {
            zoomer.tick(&mut vram, &mut palette);
        }
        for row in 0..400usize {
            let expected = if row <= 260 || row >= 274 { 255 } else { 0 };
            assert!(
                vram[row * WIDTH..(row + 1) * WIDTH]
                    .iter()
                    .all(|&p| p == expected),
                "row {row}"
            );
        }
    }

    /// The palette fade: c=0 on the first tick leaves `pal1` in place; from
    /// the 33rd tick on, c=32 makes every uploaded entry 30.
    #[test]
    fn the_zoomer_fades_the_first_128_entries_to_30() {
        let mut palette = test_palette();
        let mut vram = vec![0u8; VRAM_PIXELS];
        let mut zoomer = Zoomer::new(&palette);
        zoomer.tick(&mut vram, &mut palette);
        for index in 0..128u8 {
            assert_eq!(
                palette.entry(index),
                test_palette().entry(index),
                "entry {index}"
            );
        }
        for _ in 1..33 {
            assert!(zoomer.tick(&mut vram, &mut palette));
        }
        assert_eq!(zoomer.frames(), 33);
        for index in 0..128u8 {
            assert_eq!(palette.entry(index), [30, 30, 30], "entry {index}");
        }
        assert_eq!(
            palette.entry(128),
            test_palette().entry(128),
            "entry 128 is not uploaded"
        );
    }

    /// The epilogue: all 400 rows zero and entry 0 black.
    #[test]
    fn the_zoomer_epilogue_clears_every_row_and_entry_0() {
        let mut palette = test_palette();
        let mut vram = vec![255u8; VRAM_PIXELS];
        finish(&mut vram, &mut palette);
        assert!(vram.iter().all(|&p| p == 0));
        assert_eq!(palette.entry(0), [0, 0, 0]);
        assert_eq!(
            palette.entry(1),
            test_palette().entry(1),
            "nothing else changes"
        );
    }

    /// The spring's 177 draws and its final band (`MAIN.C:323-350`).
    #[test]
    fn the_bounce_draws_177_ticks_and_ends_on_rows_153_to_200() {
        let picture = fc_picture();
        let mut vram = vec![0u8; VRAM_PIXELS];
        let mut bounce = Bounce::new();
        let mut drawn = 0;
        while bounce.tick(&mut vram, picture.pixels()) {
            drawn += 1;
            assert!(drawn <= 200, "the spring must terminate");
        }
        assert_eq!(drawn, 177);
        assert_eq!(bounce.drawn(), 177);
        assert_eq!(bounce.band(), (153, 200));
    }

    /// The final band's pixels (`MAIN.C:337-350`): rows 149..152 black, then
    /// `fcrow[c/256]` rows 153..199, with `c` advancing by
    /// `25600/(200-153) = 544`.
    #[test]
    fn the_final_bounce_band_is_the_source_scaled_picture_rows() {
        let picture = fc_picture();
        let fc = picture.pixels();
        let mut vram = vec![0u8; VRAM_PIXELS];
        let mut bounce = Bounce::new();
        while bounce.tick(&mut vram, fc) {}
        for row in 149..153usize {
            assert!(
                vram[row * WIDTH..(row + 1) * WIDTH].iter().all(|&p| p == 0),
                "row {row} must be the black gap"
            );
        }
        let mut c = 0;
        for row in 153..200usize {
            let source = (c / 256) as usize * WIDTH;
            assert_eq!(
                &vram[row * WIDTH..(row + 1) * WIDTH],
                &fc[source..source + WIDTH],
                "row {row} (c {c})"
            );
            c += 544;
        }
    }

    /// **The acceptance test**: the zoomer's epilogue, the mode-13h clear and
    /// 177 bounce ticks leave exactly the reference's held index plane. The
    /// reference frame 1499 is the bounce's last frame (§7.3, §7.6).
    #[test]
    fn the_held_intro_is_the_reference_index_plane() {
        let picture = fc_picture();
        let mut palette = test_palette();
        let mut vram = vec![0u8; VRAM_PIXELS];

        // The zoomer's 47 ticks and its epilogue.
        let mut zoomer = Zoomer::new(&palette);
        while zoomer.tick(&mut vram, &mut palette) {}
        finish(&mut vram, &mut palette);

        // `int 10h` 13h and the source's `memset(vram,0,65535)`, which in
        // chain-4 clears the whole 400-row plane; the displayed 64,000 bytes
        // are what the bounce and the reference use.
        vram.fill(0);

        let mut bounce = Bounce::new();
        while bounce.tick(&mut vram, picture.pixels()) {}

        let reference = held_reference();
        assert_eq!(reference.len(), SCREEN_PIXELS);
        let differing = vram[..SCREEN_PIXELS]
            .iter()
            .zip(&reference)
            .filter(|(port, reference)| port != reference)
            .count();
        assert_eq!(
            differing, 0,
            "the held intro must be the reference's 64,000 indices"
        );
        // The reference's own shape, restated so a wrong file is visible:
        // content only in rows 154..199 (§7.3).
        for row in 0..154usize {
            assert!(
                reference[row * WIDTH..(row + 1) * WIDTH]
                    .iter()
                    .all(|&p| p == 0),
                "reference row {row} is black"
            );
        }
        assert!(reference[154 * WIDTH..].iter().any(|&p| p != 0));
    }
}
