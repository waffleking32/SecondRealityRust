//! The interference: `init_interference`'s planes (`KOEA.ASM`/`KOEB.ASM`), the
//! two `do_interference` loops and the EGA window they are shown through.

use super::tables::{PAL0, PAL1, PAL2};
use super::{Techno, PLANE_BYTES, PLANE_ROW_WIDE, SCREEN_HEIGHT};
use crate::parts::sin1024::SIN1024;

/// The first pass's display start: `80*scrny + scrnx/8` with `sizefade` 0,
/// so `scrnx` 160 and `scrny` 100 (`KOEB.ASM:502-590`), and its pan 0.
const FIRST_PASS_START: i32 = 80 * 100 + 160 / 8;

impl Techno {
    /// `bltline`: the 40 bytes of `src` to `mask`'s plane at a plane offset.
    fn bltline(&mut self, mask: u8, src: &[u8], dst: usize) {
        let plane = (mask.trailing_zeros() as usize).min(3);
        for (k, value) in src.iter().enumerate().take(40) {
            self.planes[plane][(dst + k) & (PLANE_BYTES - 1)] = *value;
        }
    }

    /// `bltlinerev`: the same, the 40 bytes reversed and bit-reversed.
    fn bltlinerev(&mut self, mask: u8, src: &[u8], dst: usize) {
        let plane = (mask.trailing_zeros() as usize).min(3);
        for k in 0..40usize {
            self.planes[plane][(dst + k) & (PLANE_BYTES - 1)] = self.flip8[src[39 - k] as usize];
        }
    }

    /// `init_interference`: the two circle pictures, top and bottom
    /// (vertically mirrored), forward and mirrored, then the eight rotated
    /// plane-2 copies.
    pub(super) fn init_interference(&mut self) {
        self.stride = PLANE_ROW_WIDE; // CRTC 13h = 28h
        for y in 0..200usize {
            let row: [u8; 40] = self.circle2[y * 40..(y + 1) * 40].try_into().unwrap();
            for base in [y * 80, (399 - y) * 80] {
                self.bltline(4, &row, base);
                self.bltlinerev(4, &row, base + 40);
            }
        }
        self.circles[0][..32_000].copy_from_slice(&self.planes[2][..32_000]);
        for i in 0..7usize {
            let (done, rest) = self.circles.split_at_mut(i + 1);
            rotate1(&done[i], &mut rest[0]);
        }
        for y in 0..200usize {
            let row: [u8; 120] = self.circle[y * 120..(y + 1) * 120].try_into().unwrap();
            for base in [y * 80, (399 - y) * 80] {
                for plane in 0..3usize {
                    let src = &row[plane * 40..plane * 40 + 40];
                    self.bltline(1 << plane, src, base);
                    self.bltlinerev(1 << plane, src, base + 40);
                }
            }
        }
        self.p2_framecount = 0;
    }

    /// `KOEB.ASM`'s `do_interference`, one iteration: the palette fade, the
    /// row's `patdir`, and the constant display start.
    pub(super) fn first_pass_iteration(&mut self, musrow: i32) {
        self.palanimc += self.patdir;
        if self.palanimc < 0 {
            self.palanimc = 8 * 3 - 3;
        }
        if self.palanimc >= 8 * 3 {
            self.palanimc = 0;
        }
        self.palfader = (self.palfader + 2).min(512);
        let d = self.palfader;
        let mut mix = [0u8; 48];
        for half in 0..2usize {
            for j in 0..8usize {
                let at = self.palanimc as usize + j * 3;
                for (k, &src) in PAL0[at..at + 3].iter().enumerate() {
                    let src = i32::from(src);
                    let value = if d <= 256 { (src * d) >> 8 } else { (src + d - 256).min(63) };
                    mix[half * 24 + j * 3 + k] = value as u8;
                }
            }
        }
        self.upload_dac(0, &mix);
        if musrow & 7 == 0 || musrow & 7 == 4 {
            self.patdir = -3;
        }
        self.display_start = FIRST_PASS_START;
        self.p1_framecount += 1;
    }

    /// `KOEA.ASM`'s `do_interference`, one iteration.
    pub(super) fn second_pass_iteration(&mut self, musrow: i32) {
        // The MOVE writes the start and pan after the draw, so a frame shows
        // the previous iteration's; the first frame shows the first pass's.
        if self.p2_first_frame {
            self.p2_first_frame = false;
        } else {
            self.display_start = self.p2_scrnpos;
        }
        self.pan = self.p2_scrnposl as u8;
        let at = self.p2_palanimc as usize;
        let mut bytes = [0u8; 48];
        bytes[..24].copy_from_slice(&PAL1[at..at + 24]);
        bytes[24..].copy_from_slice(&PAL2[at..at + 24]);
        self.upload_dac(0, &bytes);
        self.interference_draw();
        // The MOVE section.
        let r = musrow & 7;
        if r == 0 || r == 4 {
            self.p2_patdir = -3;
        }
        self.p2_scrnrot = (self.p2_scrnrot + 5) & 1023;
        let scrnx = ((SIN1024[self.p2_scrnrot as usize] as i32) >> 2) + 160;
        let scrny = ((SIN1024[((self.p2_scrnrot + 256) & 1023) as usize] as i32) >> 2) + 100;
        self.p2_overrot = (self.p2_overrot + 7) & 1023;
        self.p2_overx = ((SIN1024[self.p2_overrot as usize] as i32) >> 2) + 160;
        let oy = ((SIN1024[((self.p2_overrot + 256) & 1023) as usize] as i32) >> 2) + 100;
        self.p2_overya = (oy * 80) & 0xFFFF;
        self.p2_scrnposl = scrnx & 7;
        self.p2_scrnpos = 80 * scrny + (scrnx >> 3);
        // `framecount` is tested against 350 before it is incremented
        // (`KOEA.ASM:887-897`).
        if self.p2_framecount >= 70 * 5 {
            self.p2_powercnt = self.p2_powercnt.wrapping_add(1);
            if self.p2_powercnt >= 16 {
                self.p2_powercnt = 0;
                if self.p2_sinuspower < 15 {
                    self.p2_sinuspower += 1;
                }
            }
        }
        self.p2_framecount += 1;
        self.p2_palanimc += self.p2_patdir;
        if self.p2_palanimc < 0 {
            self.p2_palanimc = 8 * 3 - 3;
        }
        if self.p2_palanimc >= 8 * 3 {
            self.p2_palanimc = 0;
        }
    }

    /// The moving circles in plane 3, 44 bytes a row at a 640-wide address
    /// space (`KOEA.ASM:779-818`).
    fn interference_draw(&mut self) {
        self.p2_sinurot = (self.p2_sinurot + 14) & 2047;
        let mut bp = self.p2_sinurot;
        for row in 0..200usize {
            bp = (bp + 18) & 2047;
            // `and bp,2047` is a byte offset into the 1,024-word table, so the
            // word is `bp/2` (the MOVE routines' cursors are word indices,
            // `shl bx,1`).
            let mut bx = (SIN1024[(bp >> 1) as usize] as i32) >> 3;
            bx = (bx & 0xFF) | (i32::from(self.p2_sinuspower) << 8);
            let a = self.power0[(bx & 0xFFFF) as usize] as i32;
            let ax = a - self.p2_scrnposl + self.p2_overx;
            let which = (7 - (ax & 7)) as usize;
            // `push si` / `pop si` and `add si,80` (`KOEA.ASM:790,812-814`):
            // row `r` reads from `80*r + overya + (ax >> 3)`.
            let si = (row as i32 * 80 + self.p2_overya + (ax >> 3)) as u16 as usize;
            let di = self.p2_scrnpos as usize + row * 80;
            let circle = &self.circles[which];
            for k in 0..44usize {
                self.planes[3][(di + k) & (PLANE_BYTES - 1)] = circle[si + k];
            }
        }
    }

    /// The 320×200 window: four plane bits a pixel, the leftmost pixel bit 7.
    pub(super) fn compose_ega(&self, surface: &mut [u8]) {
        let start = self.display_start as usize;
        let pan = usize::from(self.pan);
        for y in 0..usize::from(SCREEN_HEIGHT) {
            let base = (start + y * self.stride) & (PLANE_BYTES - 1);
            let row = &mut surface[y * 640..y * 640 + 320];
            for (bit, pixel) in (pan..).zip(row.iter_mut()) {
                let at = (base + (bit >> 3)) & (PLANE_BYTES - 1);
                let shift = 7 - (bit & 7);
                *pixel = ((self.planes[0][at] >> shift) & 1)
                    | (((self.planes[1][at] >> shift) & 1) << 1)
                    | (((self.planes[2][at] >> shift) & 1) << 2)
                    | (((self.planes[3][at] >> shift) & 1) << 3);
            }
        }
    }
}

/// `rotate1` (`KOEA.ASM:605-627`): rotate the first 31,936 bytes right one bit
/// with a zero shifted in; the last 64 bytes of `dst` are untouched.
fn rotate1(src: &[u8], dst: &mut [u8]) {
    let mut carry = 0u8;
    for i in 0..32_000 - 64 {
        let value = src[i];
        dst[i] = (value >> 1) | (carry << 7);
        carry = value & 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Blank planes and the second pass's statics at 0.
    fn second_pass_at_zero(sinuspower: u8) -> Techno {
        let mut t = Techno::new();
        t.p2_sinuspower = sinuspower;
        t
    }

    #[test]
    fn the_first_pass_reaches_white_at_its_two_hundred_and_first_iteration() {
        let mut t = Techno::new();
        for _ in 0..101 {
            t.first_pass_iteration(0);
        }
        // 2*101 = 202: pal0's cyan scaled by 202/256; palanimc 9 puts it at
        // entries 5 and 13.
        assert_eq!((t.palfader, t.palanimc), (202, 9));
        let cyan = [0, (30 * 202 / 256) as u8, (40 * 202 / 256) as u8];
        assert_eq!(&t.dac[15..18], &cyan);
        assert_eq!(&t.dac[39..42], &cyan);
        assert!(t.dac[..15].iter().chain(&t.dac[18..39]).chain(&t.dac[42..48]).all(|v| *v == 0));
        for _ in 101..201 {
            t.first_pass_iteration(0);
        }
        // 402 - 256 = 146 added to every channel, clamped: white.
        assert_eq!(t.palfader, 402);
        assert!(t.dac[..48].iter().all(|v| *v == 63), "the fade is white at iteration 201");
        assert_eq!(t.display_start, FIRST_PASS_START);
        for _ in 201..256 {
            t.first_pass_iteration(0);
        }
        assert_eq!(t.palfader, 512, "capped at 512");
    }

    #[test]
    fn the_second_pass_circle_walks_one_eighty_byte_row_a_row() {
        // `power0`'s row 0 is all zeros, so every row's `ax` is 0 and the
        // window is `circles[7]`; each source byte carries its own row.
        let mut t = second_pass_at_zero(0);
        t.circles[7] = (0..32_768usize).map(|i| (i / 80) as u8).collect();
        t.interference_draw();
        for row in 0..200usize {
            for k in 0..44usize {
                assert_eq!(t.planes[3][row * 80 + k], row as u8, "row {row} byte {k}");
            }
        }
    }

    #[test]
    fn the_second_pass_reads_the_sine_as_a_byte_offset() {
        let mut t = second_pass_at_zero(1);
        for (k, circle) in t.circles.iter_mut().enumerate() {
            circle.fill(0x10 + k as u8);
        }
        t.interference_draw();
        let mut selected = std::collections::BTreeSet::new();
        for row in 0..200usize {
            // `sinurot += 14`, then `bp += 18` a row (`KOEA.ASM:785,791`);
            // the word is `bp/2`.
            let bp = (14 + 18 * (row + 1)) & 2047;
            let bx = ((SIN1024[bp / 2] as i32 >> 3) & 0xFF) | (1 << 8);
            let ax = i32::from(t.power0[bx as usize]);
            let which = (7 - (ax & 7)) as usize;
            selected.insert(which);
            let si = (row as i32 * 80 + (ax >> 3)) as u16 as usize;
            for k in 0..44usize {
                assert_eq!(t.planes[3][row * 80 + k], t.circles[which][si + k], "row {row}");
            }
        }
        assert!(selected.len() > 1, "sinuspower 1 selects more than one circle");
    }

    #[test]
    fn rotate1s_last_sixty_four_bytes_are_untouched() {
        let mut src = vec![0u8; 32_000];
        src[0] = 0x03;
        src[31_935] = 0xFF;
        let mut dst = vec![0xAAu8; 32_000];
        rotate1(&src, &mut dst);
        assert_eq!(&dst[..2], &[0x01, 0x80], "a zero in, bit 0 carried to the next byte");
        assert_eq!(dst[31_935], 0x7F);
        assert!(dst[32_000 - 64..].iter().all(|byte| *byte == 0xAA), "the loop stops 64 short");
    }
}
