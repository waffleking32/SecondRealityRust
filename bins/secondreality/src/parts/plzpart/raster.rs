//! The cube's rasteriser and clear: `do_poly` (`PLZFILL.C:82-176`) over
//! `PLZA.ASM`'s `do_block` (`:59-259`) and `do_clear` (`:291-372`), writing
//! plane bytes through the periodic masks.

use super::Plzpart;

/// `TWEAK.ASM:34-35`: CRTC 0x13 = 0x50 → a 160-byte plane row.
pub(super) const CUBE_ROW_BYTES: usize = 160;
/// `PLZFILL.C:22,67`: the clear's eight rotation slots.
pub(super) const CLR_SLOTS: usize = 8;
/// The three draw buffers, in page order, 0x5500 bytes (136 rows) apart
/// (`VECT.C:267-297`).
pub(super) const PAGE_VMEM: [u16; 6] = [0x0000, 0x5500, 0xAA00, 0x0000, 0x5500, 0xAA00];
/// `PLZA.ASM:77` `cmp yy,134`: the rasteriser clips at the screen's 134 rows.
const CUBE_DRAWN_ROWS: i32 = 134;
/// The clear's walk (`ycnt=134`, mask 134): one line more than the rows drawn.
const CLEAR_LINES: i32 = CUBE_DRAWN_ROWS + 1;

/// `PLZFILL.C:88`: the texture rectangle.
const TEXTURE_POINTS: [[i32; 2]; 4] = [[64, 4], [190, 4], [190, 60], [64, 60]];

/// `ASMYT.ASM`'s two 200-entry periodic mask tables: `start_mask = 1111b,
/// 1110b, 1100b, 1000b` and `end_mask = 0001b, 0011b, 0111b, 1111b`
/// (`PLZA.ASM:13-20`), flattened so `do_clear`'s `end_mask[x-1]` at x = 0 reads
/// the byte before `end_mask` — `start_mask`'s last entry, `0x08` — exactly
/// as the original does.
const MASK_TABLE_LEN: usize = 1_600;
pub(super) const START_MASK_AT: usize = 0;
pub(super) const END_MASK_AT: usize = 800;

/// The periodic mask tables (`PLZA.ASM:13-20`), flattened so that
/// `end_mask[x-1]` at `x = 0` reads the byte before `end_mask` —
/// `start_mask`'s last entry, `0x08` (its final `1000b`), not the `0x0F` the
/// periodic pattern would give at `x = 3`.
pub(super) fn build_masks() -> Vec<u8> {
    let mut masks = vec![0u8; MASK_TABLE_LEN];
    let start = [0b1111u8, 0b1110, 0b1100, 0b1000];
    let end = [0b0001u8, 0b0011, 0b0111, 0b1111];
    for i in 0..200 {
        masks[START_MASK_AT + i * 4..START_MASK_AT + i * 4 + 4].copy_from_slice(&start);
        masks[END_MASK_AT + i * 4..END_MASK_AT + i * 4 + 4].copy_from_slice(&end);
    }
    masks
}

/// `PLZA.ASM`'s span globals: the two edges' 16.16 positions and texture
/// coordinates with their per-row steps, and where the span writes and reads.
#[derive(Default)]
pub(super) struct Span {
    pub(super) ryy: i32,
    xx1: i32,
    xx2: i32,
    txx1: i32,
    txy1: i32,
    txx2: i32,
    txy2: i32,
    ax1: i32,
    ax2: i32,
    tax1: i32,
    tay1: i32,
    tax2: i32,
    tay2: i32,
    to_row: i32,
    /// The texture (`kuva1..3`) the face reads.
    pub(super) from_texture: usize,
    /// The warp's `dist1` row, `dd`.
    pub(super) dseg_row: usize,
}

impl Plzpart {
    /// `PLZFILL.C:82-176` `do_poly`: the span setup.
    pub(super) fn do_poly(&mut self, pnts: [(i32, i32); 4], color: usize, dd: i32) {
        let dd = (dd + 1) & 63;
        let txt = TEXTURE_POINTS;
        let mut n = 0usize;
        for a in 1..4 {
            if pnts[a].1 < pnts[n].1 {
                n = a;
            }
        }
        let mut s1 = n;
        let mut s2 = n;
        let mut d1 = (s1 + 1) & 3;
        let mut d2 = (s2 + 3) & 3;
        self.setup_edge(&pnts, &txt, s1, d1, 1);
        self.setup_edge(&pnts, &txt, s2, d2, 2);
        self.span.ryy = pnts[s1].1;
        self.span.from_texture = color;
        self.span.dseg_row = dd as usize;
        self.span.to_row = i32::from(PAGE_VMEM[self.page]) + self.span.ryy * CUBE_ROW_BYTES as i32;
        let mut count = 0;
        while count < 4 {
            let m = pnts[d1].1.min(pnts[d2].1);
            self.do_block(m - self.span.ryy);
            self.span.ryy = m;
            if pnts[d1].1 == pnts[d2].1 {
                s1 = d1;
                d1 = (s1 + 1) & 3;
                s2 = d2;
                d2 = (s2 + 3) & 3;
                count += 2;
                self.setup_edge(&pnts, &txt, s1, d1, 1);
                self.setup_edge(&pnts, &txt, s2, d2, 2);
            } else if pnts[d1].1 < pnts[d2].1 {
                s1 = d1;
                d1 = (s1 + 1) & 3;
                count += 1;
                self.setup_edge(&pnts, &txt, s1, d1, 1);
            } else {
                s2 = d2;
                d2 = (s2 + 3) & 3;
                count += 1;
                self.setup_edge(&pnts, &txt, s2, d2, 2);
            }
        }
    }

    fn setup_edge(
        &mut self,
        pnts: &[(i32, i32); 4],
        txt: &[[i32; 2]; 4],
        s: usize,
        d: usize,
        which: usize,
    ) {
        let dx = pnts[d].0.wrapping_sub(pnts[s].0);
        let mut dy = pnts[d].1.wrapping_sub(pnts[s].1);
        if dy == 0 {
            dy = 1;
        }
        let slope = 65_536i32.wrapping_mul(dx) / dy;
        let xx = pnts[s].0.wrapping_shl(16).wrapping_add(0x8000);
        let txx = txt[s][0].wrapping_shl(16).wrapping_add(0x8000);
        let txy = txt[s][1].wrapping_shl(16).wrapping_add(0x8000);
        let tax = 65_536i32.wrapping_mul(txt[d][0].wrapping_sub(txt[s][0])) / dy;
        let tay = 65_536i32.wrapping_mul(txt[d][1].wrapping_sub(txt[s][1])) / dy;
        if which == 1 {
            self.span.ax1 = slope;
            self.span.xx1 = xx;
            self.span.txx1 = txx;
            self.span.txy1 = txy;
            self.span.tax1 = tax;
            self.span.tay1 = tay;
        } else {
            self.span.ax2 = slope;
            self.span.xx2 = xx;
            self.span.txx2 = txx;
            self.span.txy2 = txy;
            self.span.tax2 = tax;
            self.span.tay2 = tay;
        }
    }

    /// One byte to all selected planes at a 16-bit plane-byte address.
    fn write_plane_byte(&mut self, address: u16, mask: u8, value: u8) {
        for plane in 0..4usize {
            if mask & (1 << plane) != 0 {
                self.video[4 * address as usize + plane] = value;
            }
        }
    }

    /// The clock at a texture address: `dist1[dd*256 + bx]` (unsigned) and
    /// then the texture byte at `bx + dist` (`PLZA.ASM:130-166`).
    pub(super) fn texture_sample(&self, bx: u16) -> u8 {
        // `dseg = disttaus[color] + dd*16` adds **16 paragraphs** (256 bytes)
        // to the segment, so the warp shifts `dist1` by `dd` 256-byte rows.
        let dist = self.dist1[self.span.dseg_row * 256 + bx as usize];
        let texture = match self.span.from_texture {
            0 => &self.kuva1,
            1 => &self.kuva2,
            _ => &self.kuva3,
        };
        let address = bx.wrapping_add(u16::from(dist)) as usize;
        if address >= texture.len() {
            // The texture is 128×256 and the dist warp stays in its half;
            // a read past it is the original's segment wrap, not a sample.
            return 0;
        }
        texture[address]
    }

    /// `PLZA.ASM:59-259` `do_block`.
    fn do_block(&mut self, ycount: i32) {
        if ycount == 0 {
            return;
        }
        let mut lines = ycount;
        let mut to = self.span.to_row;
        loop {
            if self.span.ryy >= 0 && self.span.ryy < CUBE_DRAWN_ROWS {
                self.block_line(to as u16);
            } else if self.span.ryy >= CUBE_DRAWN_ROWS {
                break;
            }
            to = to.wrapping_add(CUBE_ROW_BYTES as i32);
            self.span.ryy += 1;
            self.span.xx1 = self.span.xx1.wrapping_add(self.span.ax1);
            self.span.xx2 = self.span.xx2.wrapping_add(self.span.ax2);
            self.span.txy1 = self.span.txy1.wrapping_add(self.span.tay1);
            self.span.txx1 = self.span.txx1.wrapping_add(self.span.tax1);
            self.span.txy2 = self.span.txy2.wrapping_add(self.span.tay2);
            self.span.txx2 = self.span.txx2.wrapping_add(self.span.tax2);
            lines -= 1;
            if lines == 0 {
                break;
            }
        }
        self.span.to_row = to;
    }

    /// One scanline's span (`PLZA.ASM:73-192`).
    fn block_line(&mut self, to: u16) {
        let row = self.span.ryy as usize;
        let right = (self.span.xx2 as u32 >> 16) as u16;
        let left = (self.span.xx1 as u32 >> 16) as u16;
        // `PLZA.ASM:73-90`: `ctau[0]` is the *smallest* span start (`cmp ax,
        // [gs:bp]` / `ja` keeps the smaller) and `ctau[1]` the *largest* span
        // end — the row's union hull, which is what `do_clear` trims against.
        let entry = self.clrptr * 256 + row;
        if right <= self.clrtau[entry][0] as u16 {
            self.clrtau[entry][0] = i32::from(right);
        }
        if left >= self.clrtau[entry][1] as u16 {
            self.clrtau[entry][1] = i32::from(left);
        }
        let start_col = right >> 2;
        let end_col = left >> 2;
        if end_col < start_col {
            return;
        }
        let bytes = end_col - start_col;
        let di = to.wrapping_add(start_col);
        if bytes == 0 {
            // `@@singlebyte`: the start and end masks combined.
            let sample = self.texture_sample(texture_address(self.span.txy2, self.span.txx2));
            let mask = self.masks[START_MASK_AT + usize::from(right & 3)]
                & self.masks[END_MASK_AT + usize::from(left & 3)];
            self.write_plane_byte(di, mask, sample);
            return;
        }
        // `@@twobyte`: the two end bytes first.
        let cl = self.texture_sample(texture_address(self.span.txy2, self.span.txx2));
        let ch = self.texture_sample(texture_address(self.span.txy1, self.span.txx1));
        self.write_plane_byte(di, self.masks[START_MASK_AT + usize::from(right & 3)], cl);
        self.write_plane_byte(
            di.wrapping_add(bytes),
            self.masks[END_MASK_AT + usize::from(left & 3)],
            ch,
        );
        if bytes >= 2 {
            self.fill_span(di, bytes);
        }
    }

    /// `PLZA.ASM:125-166`: the span's middle bytes, right to left.
    fn fill_span(&mut self, di: u16, bytes: u16) {
        // `mov bp,si; dec si; ... movsx esi,si`: the divisor is **bytes-1**,
        // the count of middle bytes, not the span length.
        let divisor = bytes as i32 - 1;
        let addy = self.span.txy2.wrapping_sub(self.span.txy1) / divisor;
        let addx = self.span.txx2.wrapping_sub(self.span.txx1) / divisor;
        // `mov ebx,[txy1-2]`: the y fraction in the high word, tx in the low.
        let mut ebx = (self.span.txy1 as u32 & 0xFFFF) << 16 | ((self.span.txx1 as u32 >> 16) & 0xFFFF);
        let mut ecx = addy as u32;
        let eax = (addx as u32).rotate_left(16);
        let rotated = (addy as u32).rotate_left(16);
        // `mov dh,dl`: dh becomes the rotated low byte.
        let mut dh = (rotated & 0xFF) as u8;
        // `mov dl,al`: dl becomes the rotated addx's low byte.
        let dl = (eax & 0xFF) as u8;
        let mut edx = (rotated & 0xFFFF_0000) | ((dh as u32) << 8) | (dl as u32);
        if dl & 0x80 != 0 {
            dh = dh.wrapping_sub(1);
            // Clear both bytes dh occupies: the mask must keep only the top
            // 16 bits (the y fraction); `0xFFFF_FF00` would leave the old
            // dh behind and OR the corrected one into it.
            edx = (edx & 0xFFFF_00FF) | ((dh as u32) << 8) | (dl as u32);
        }
        // The address bytes come from `txy1+2` / `txx1+2`.
        ebx = (ebx & 0xFFFF_0000)
            | (((self.span.txy1 as u32 >> 16) & 0xFF) << 8)
            | ((self.span.txx1 as u32 >> 16) & 0xFF);
        // The middle bytes, right to left (`PLZA.ASM:148-166`).
        let mut position = di.wrapping_add(bytes).wrapping_sub(1);
        for _step in 0..bytes.saturating_sub(1) {
            let bx = ebx as u16;
            let sample = self.texture_sample(bx);
            self.write_plane_byte(position, 0x0F, sample);
            position = position.wrapping_sub(1);
            // `add ecx,eax` sets the carry the following `adc ebx,edx` uses.
            let (sum, carry1) = ecx.overflowing_add(eax);
            ecx = sum;
            let (sum, carry2a) = ebx.overflowing_add(edx);
            let (sum, carry2b) = sum.overflowing_add(u32::from(carry1));
            ebx = sum;
            let carry = carry2a || carry2b;
            // `adc bh,0`: the carry out of the 32-bit sum lands in ty.
            let bh = ((ebx >> 8) & 0xFF) as u8;
            ebx = (ebx & 0xFFFF_00FF) | (u32::from(bh.wrapping_add(u8::from(carry))) << 8);
        }
    }

    /// `PLZFILL.C:178-185` `clear`.
    pub(super) fn clear(&mut self) {
        let old = (self.clrptr + CLR_SLOTS - 3) % CLR_SLOTS;
        let new = self.clrptr;
        self.clrptr = (self.clrptr + 1) % CLR_SLOTS;
        self.do_clear(old, new);
    }

    /// `PLZA.ASM:291-372` `do_clear`.
    fn do_clear(&mut self, old_slot: usize, new_slot: usize) {
        // `les dx,[vmem]` puts the drawn page's offset in dx, then
        // `mov dl,[es:0]` overwrites its low byte with the byte at the start
        // of plane memory. The original's bug; kept. The high byte still
        // carries the page (0x5500/0xAA00), which is what aims the fill at
        // the buffer the draw used.
        let mut dx = (PAGE_VMEM[self.page] & 0xFF00) | u16::from(self.video[0]);
        for line in 0..CLEAR_LINES as usize {
            let old = self.clrtau[old_slot * 256 + line];
            let new = self.clrtau[new_slot * 256 + line];
            if old[0] != 640 {
                let from = (old[0] as u16) >> 2;
                let to = (new[0] as u16) >> 2;
                if to >= from {
                    let mut at = from.wrapping_add(dx);
                    for _ in 0..(to - from) {
                        self.write_plane_byte(at, 0x0F, 0);
                        at = at.wrapping_add(1);
                    }
                }
                let from = (new[1] as u16) >> 2;
                let to = (old[1] as u16) >> 2;
                if to >= from {
                    let mut at = from.wrapping_add(dx).wrapping_add(1);
                    for _ in 0..(to - from) {
                        self.write_plane_byte(at, 0x0F, 0);
                        at = at.wrapping_add(1);
                    }
                }
            }
            self.clrtau[old_slot * 256 + line] = [640, 0];
            dx = dx.wrapping_add(CUBE_ROW_BYTES as u16);
        }
        // `PLZA.ASM:345-368`: the two edge bytes of each span. `les di,[vmem]`
        // starts the pointer at the drawn page, then `add di,160` a line.
        let mut offset = PAGE_VMEM[self.page];
        for line in 0..(CLEAR_LINES - 1) as usize {
            let new = self.clrtau[new_slot * 256 + line];
            // `end_mask[x-1]` — at x = 0 the 16-bit displacement wraps to the
            // byte before end_mask, which is start_mask's last 0x08.
            let x = (new[0] as u16).wrapping_sub(1);
            let mask = self.masks[(END_MASK_AT as u16).wrapping_add(x) as usize];
            self.write_plane_byte(offset.wrapping_add(x >> 2), mask, 0);
            let x = (new[1] as u16).wrapping_add(1);
            let mask = self.masks[(START_MASK_AT as u16).wrapping_add(x) as usize];
            self.write_plane_byte(offset.wrapping_add(x >> 2), mask, 0);
            offset = offset.wrapping_add(CUBE_ROW_BYTES as u16);
        }
    }
}

/// The texture address `bh<<8 | bl` from a 16.16 pair (`PLZA.ASM:125-126`).
fn texture_address(txy: i32, txx: i32) -> u16 {
    (((txy as u32 >> 16) & 0xFF) << 8 | ((txx as u32 >> 16) & 0xFF)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parts::plzpart::cube::VIDEO_BYTES;

    /// One draw buffer's size in the linear image: 0x5500 plane bytes (136 rows).
    const PAGE_SLOT_LINEAR: usize = 4 * 0x5500;

    /// A part on page 0 with a cleared image, a flat texture 7 and no warp.
    fn raster_part() -> Plzpart {
        let mut part = Plzpart::new();
        part.video = vec![0; VIDEO_BYTES];
        part.kuva1 = vec![7; 16_384];
        part.dist1 = vec![0; 32_768];
        part
    }

    /// Pixel `(x, y)` of page 0's buffer.
    fn pixel(part: &Plzpart, x: usize, y: usize) -> u8 {
        part.video[(y * CUBE_ROW_BYTES + x / 4) * 4 + (x & 3)]
    }

    /// The drawn pixels' bounding box on page 0: `(minx, maxx, miny, maxy)`.
    fn drawn_bounds(part: &Plzpart) -> (usize, usize, usize, usize) {
        let mut bounds = (usize::MAX, 0, usize::MAX, 0);
        for y in 0..200 {
            for x in 0..639 {
                if pixel(part, x, y) != 0 {
                    bounds = (bounds.0.min(x), bounds.1.max(x), bounds.2.min(y), bounds.3.max(y));
                }
            }
        }
        bounds
    }

    #[test]
    fn do_clears_mask_underflow_reads_the_byte_before_end_mask() {
        let masks = build_masks();
        // The table's last entry is 1000b, not 1111b: the underflow reads 8
        // where a masked index would read 15 — the preserved bug.
        assert_eq!(masks[END_MASK_AT - 1], 0b1000);
        assert_eq!(masks[END_MASK_AT + 3], 0b1111);
        assert_ne!(masks[END_MASK_AT - 1], masks[END_MASK_AT + 3]);
        let start = [0b1111u8, 0b1110, 0b1100, 0b1000];
        let end = [0b0001u8, 0b0011, 0b0111, 0b1111];
        for i in 0..200 {
            for j in 0..4 {
                assert_eq!(masks[START_MASK_AT + i * 4 + j], start[j]);
                assert_eq!(masks[END_MASK_AT + i * 4 + j], end[j]);
            }
        }
    }

    /// A draw taller than the screen stays inside its own 0x5500-byte slot: no
    /// write reaches the neighbouring buffer.
    #[test]
    fn a_134_row_draw_stays_inside_its_slot() {
        for (page, &vmem) in PAGE_VMEM.iter().enumerate().take(3) {
            let mut part = raster_part();
            part.page = page;
            // A quad from above the top to far below the foot of any screen.
            part.do_poly([(10, -20), (300, -20), (300, 250), (10, 250)], 0, 0);
            let slot = usize::from(vmem) * 4;
            let mut inside = 0usize;
            for (linear, &byte) in part.video.iter().enumerate() {
                if byte != 0 {
                    assert!(
                        (slot..slot + PAGE_SLOT_LINEAR).contains(&linear),
                        "page {page}: a write at linear {linear:#x} left its slot \
                         {slot:#x}..{:#x}",
                        slot + PAGE_SLOT_LINEAR
                    );
                    inside += 1;
                }
            }
            assert!(inside > 134 * 280, "page {page}: the quad drew ({inside})");
            // The last drawn row is 133; row 134 is untouched.
            let row_end = |row: usize| slot + (row + 1) * 640;
            assert!(part.video[row_end(133) - 640..row_end(133)].iter().any(|&b| b != 0));
            assert!(part.video[row_end(134) - 640..row_end(134)].iter().all(|&b| b == 0));
        }
    }

    #[test]
    fn a_synthetic_square_is_rasterised_where_its_vertices_are() {
        let mut part = raster_part();
        part.do_poly([(10, 10), (20, 10), (20, 20), (10, 20)], 0, 0);
        let pixels = |x: usize, y: usize| pixel(&part, x, y);
        let mut missed = Vec::new();
        for y in 10..20 {
            for x in 10..20 {
                if pixels(x, y) != 7 {
                    missed.push((x, y));
                }
            }
        }
        assert!(missed.len() < 10, "missed {missed:?}");
        assert_eq!(pixels(0, 0), 0);
        assert_eq!(pixels(300, 100), 0);
    }

    #[test]
    fn a_quad_is_rasterised_inside_its_edges() {
        // A slanted face, and a sliver reaching past the 320-pixel row.
        for (quad, (x0, x1, y0, y1)) in [
            ([(313, 63), (330, 54), (308, 168), (273, 198)], (270, 335, 50, 199)),
            ([(261, 22), (370, 29), (435, 42), (257, 32)], (255, 437, 20, 44)),
        ] {
            let mut part = raster_part();
            part.do_poly(quad, 0, 0);
            let (minx, maxx, miny, maxy) = drawn_bounds(&part);
            assert!(minx >= x0 && maxx <= x1, "{quad:?}: x {minx}..{maxx}");
            assert!(miny >= y0 && maxy <= y1, "{quad:?}: y {miny}..{maxy}");
        }
    }

    #[test]
    fn the_texture_runs_left_to_right_across_a_span() {
        let mut part = raster_part();
        // x = 64..190 maps to 0..126; make the texture a row gradient.
        let mut kuva = vec![0u8; 16_384];
        for y in 0..64 {
            for x in 0..256 {
                kuva[y * 256 + x] = x as u8;
            }
        }
        part.kuva1 = kuva;
        part.do_poly([(10, 10), (200, 10), (200, 20), (10, 20)], 0, 0);
        let pixels = |x: usize, y: usize| pixel(&part, x, y);
        let left = pixels(12, 15);
        let middle = pixels(100, 15);
        let right = pixels(196, 15);
        assert!(
            left < middle && middle < right,
            "the texture x increases to the right"
        );
    }

    #[test]
    fn the_texture_warp_shifts_dist1_by_dd_paragraphs() {
        // `PLZFILL.C:122` `dseg = disttaus[color] + dd*16` adds 16 paragraphs
        // (256 bytes) to the segment, so `[fs:bx]` reads `dist1[dd*256 + bx]`.
        let mut part = Plzpart::new();
        let mut kuva = vec![0u8; 16_384];
        for (index, value) in kuva.iter_mut().enumerate() {
            *value = index as u8;
        }
        part.kuva1 = kuva;
        let mut dist = vec![0u8; 32_768];
        dist[0x0102] = 5; // dd = 0
        dist[0x0102 + 256] = 9; // dd = 1 (the paragraph shift)
        part.dist1 = dist;
        // The identity texture makes the sample the low byte of bx + dist:
        // 0x0102 + 5 = 0x0107 -> 0x07.
        assert_eq!(part.texture_sample(0x0102), 0x07);
        part.span.dseg_row = 1;
        // dd = 1 reads dist1[0x0102 + 256] = 9: 0x0102 + 9 = 0x010B -> 0x0B.
        assert_eq!(part.texture_sample(0x0102), 0x0B);
    }

    #[test]
    fn the_clear_erases_a_moved_quad() {
        let mut part = raster_part();
        // Four draws: the clear erases the extents three draws back, so the
        // first square's rows go on the fourth draw.
        for y in [10usize, 40, 40, 40] {
            part.do_poly(
                [
                    (10, y as i32),
                    (100, y as i32),
                    (100, y as i32 + 10),
                    (10, y as i32 + 10),
                ],
                0,
                0,
            );
            part.clear();
        }
        let pixels = |x: usize, y: usize| pixel(&part, x, y);
        // The first square's rows must be gone; the last one's present.
        let mut stale = 0;
        for y in 10..20 {
            for x in 10..100 {
                if pixels(x, y) != 0 {
                    stale += 1;
                }
            }
        }
        assert_eq!(stale, 0, "the moved square's old rows are not erased");
        assert_ne!(pixels(50, 45), 0, "the current square is drawn");
    }
}
