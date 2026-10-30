//! The doit boxes: `POLYCLIP.ASM`'s clipper, `KOEA.ASM`'s `drawline` and
//! `_asmbox` into the 1-bit `vbuf`, and `blit16`'s prefix-XOR fill.

use super::{Techno, PLANE_BYTES, SCREEN_HEIGHT};

type Point = (i16, i16);

/// `clipmacro`: move `p`'s `.1` to the clip line `wl` along the line to
/// `other`, in the source's 16/32-bit arithmetic.
fn clip_endpoint(p: &mut Point, other: Point, wl: i16) {
    if p.1 != other.1 {
        let num = (i32::from(other.0) - i32::from(p.0)) * (i32::from(wl) - i32::from(p.1));
        let q = num / (i32::from(other.1) - i32::from(p.1));
        p.0 = (p.0 as i32).wrapping_add(q) as i16;
    }
    p.1 = wl;
}

/// The two clip passes: `clipliney` against y = 0..199, `cliplinex` against
/// x = 0..319 (the same code on swapped coordinates).
#[derive(Clone, Copy)]
enum Axis {
    Y,
    X,
}

impl Axis {
    /// The point with the clipped coordinate as `.1`, and back.
    fn swap(self, p: Point) -> Point {
        match self {
            Axis::Y => p,
            Axis::X => (p.1, p.0),
        }
    }

    fn max(self) -> i16 {
        match self {
            Axis::Y => 199,
            Axis::X => 319,
        }
    }

    /// `clipliney`/`cliplinex`: the line clipped against `0..=max`, or `None`
    /// (the source's non-zero return) when both ends are beyond one limit.
    fn clip(self, a: Point, b: Point) -> Option<(Point, Point)> {
        let max = self.max();
        let (mut a, mut b) = (self.swap(a), self.swap(b));
        let outcode = |w: i16| -> u8 {
            if w < 0 {
                1
            } else if w > max {
                2
            } else {
                0
            }
        };
        let (bl, bh) = (outcode(a.1), outcode(b.1));
        if bl & bh != 0 {
            return None;
        }
        // The second end clips against the first as it now is.
        match bl {
            1 => clip_endpoint(&mut a, b, 0),
            2 => clip_endpoint(&mut a, b, max),
            _ => {}
        }
        match bh {
            1 => clip_endpoint(&mut b, a, 0),
            2 => clip_endpoint(&mut b, a, max),
            _ => {}
        }
        Some((self.swap(a), self.swap(b)))
    }

    /// One `clipanypoly` pass over a closed polygon: the closing edge first,
    /// whose result is the first edge (`cap3`'s `jmp cap35` skips the first
    /// iteration's clipping), then the others; a point equal to the last one
    /// appended is dropped. Returns the points and the last one appended.
    fn pass(self, points: &[Point]) -> (Vec<Point>, Point) {
        let mut out = Vec::with_capacity(32);
        let mut last = (i16::MIN, i16::MIN);
        let n = points.len();
        let edges = std::iter::once((points[n - 1], points[0]))
            .chain((1..n).map(|i| (points[i - 1], points[i])));
        for (a, b) in edges {
            if let Some((a, b)) = self.clip(a, b) {
                for p in [a, b] {
                    if p != last {
                        out.push(p);
                        last = p;
                    }
                }
            }
        }
        (out, last)
    }
}

impl Techno {
    /// `cap2r`: clip one line in y then x, writing `polyxy` and `polysides`.
    fn clip_line(&mut self, a: Point, b: Point) {
        let Some((a, b)) = Axis::Y.clip(a, b).and_then(|(a, b)| Axis::X.clip(a, b)) else {
            self.polysides = 0;
            return;
        };
        self.polyxy[0] = a;
        self.polyxy[1] = b;
        self.polysides = if a == b { 1 } else { 2 };
    }

    /// `clipanypoly` (`POLYCLIP.ASM`): the y pass into `clipxy2`, then either
    /// `cap2r` on a line or the x pass into `polyxy`.
    fn clipanypoly(&mut self, pts: &[Point; 4]) {
        let (out, last) = Axis::Y.pass(pts);
        for (slot, point) in self.clipxy2.iter_mut().zip(&out) {
            *slot = *point;
        }
        let mut cx = out.len();
        if cx > 0 && self.clipxy2[0] == last {
            cx -= 1;
        }
        if cx == 0 {
            self.polysides = 0;
            return;
        }
        if cx <= 2 {
            // `clipxy2`'s second slot is stale when only one point survived.
            self.clip_line(self.clipxy2[0], self.clipxy2[1]);
            return;
        }
        let (out, last) = Axis::X.pass(&self.clipxy2[..cx]);
        let mut n = out.len();
        if n > 0 && out[0] == last {
            n -= 1;
        }
        self.polysides = n;
        self.polyxy[..n].copy_from_slice(&out[..n]);
    }

    /// `drawline` (`KOEA.ASM:170-370`): the source's y-sorted DDA with its
    /// unrolled mask states, XORed into `vbuf`.
    fn drawline(&mut self, a: Point, b: Point) {
        let (mut ax, mut bx) = (i32::from(a.0), i32::from(a.1));
        let (mut dx, mut cx) = (i32::from(b.0), i32::from(b.1));
        if bx == cx {
            return;
        }
        if bx > cx {
            std::mem::swap(&mut bx, &mut cx);
            std::mem::swap(&mut ax, &mut dx);
        }
        let dy = cx - bx;
        let mut si = (bx * 40 + (ax as u32 >> 3) as i32) & 0xFFFF;
        let right = ax < dx;
        let x_total = if right { dx - ax } else { ax - dx };
        let mut acc = -(dy >> 1);
        let mut state = ax & 7;
        let mut count = dy;
        loop {
            let at = (si & 0xFFFF) as usize;
            self.vbuf[at] ^= 0x80u8 >> state;
            si = (si + 40) & 0xFFFF;
            count -= 1;
            if count == 0 {
                return;
            }
            acc += x_total;
            if acc < 0 {
                continue;
            }
            loop {
                if right {
                    state = (state + 1) & 7;
                    if state == 0 {
                        si = (si + 1) & 0xFFFF;
                    }
                } else {
                    state = (state + 7) & 7;
                    if state == 7 {
                        si = (si - 1) & 0xFFFF;
                    }
                }
                acc -= dy;
                if acc < 0 {
                    break;
                }
            }
        }
    }

    /// `_asmbox` (`KOEA.ASM:373-419`): clip the four points and draw the
    /// polygon's edges. A one-point result draws `polyxy[0]` to itself, which
    /// `drawline` skips (equal rows).
    fn asmbox(&mut self, pts: [Point; 4]) {
        self.clipanypoly(&pts);
        let n = self.polysides;
        if n <= 1 {
            return;
        }
        for i in 0..n - 1 {
            self.drawline(self.polyxy[i], self.polyxy[i + 1]);
        }
        self.drawline(self.polyxy[n - 1], self.polyxy[0]);
    }

    /// The eleven parallelograms a `doit` frame draws about `(cx0, cy0)`.
    pub(super) fn boxes(&mut self, hx: i32, hy: i32, vx: i32, vy: i32, cx0: i32, cy0: i32) {
        for c in (-10..11).step_by(2) {
            let cx = vx * c * 2;
            let cy = vy * c * 2;
            let corner = |sh: i32, sv: i32| -> Point {
                let x = (sh * hx + sv * vx + cx) / 16 + cx0;
                let y = (sh * hy + sv * vy + cy) / 16 + cy0;
                (x as i16, y as i16)
            };
            self.asmbox([corner(-1, -1), corner(-1, 1), corner(1, 1), corner(1, -1)]);
        }
    }

    /// `blit16`/`blit16b`: the prefix-XOR fill of `vbuf` into one plane.
    pub(super) fn blit(&mut self, page: usize, mask: u8, row_step: usize) {
        let plane = (mask.trailing_zeros() as usize).min(3);
        for y in 0..usize::from(SCREEN_HEIGHT) {
            let mut carry = 0u8;
            for k in 0..40usize {
                let e = self.vbuf[y * 40 + k] ^ carry;
                let (lo, hi) = self.blit[e as usize];
                let at = (page + y * row_step + k) & (PLANE_BYTES - 1);
                self.planes[plane][at] = lo;
                carry = hi;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::PLANE_ROW;
    use super::*;

    #[test]
    fn the_box_renderer_fills_the_clipped_polygon() {
        let mut t = Techno::new();
        t.asmbox([(10, 10), (30, 10), (30, 30), (10, 30)]);
        t.blit(0, 1, PLANE_ROW);
        let bit = |x: usize, y: usize| -> u8 { (t.planes[0][y * 40 + x / 8] >> (7 - (x & 7))) & 1 };
        // `drawline` skips horizontal edges and drops its last row: the
        // even-odd fill is rows 10..30, columns 10..30.
        for y in 10..30usize {
            for x in 10..30usize {
                assert_eq!(bit(x, y), 1, "the interior pixel ({x},{y})");
            }
        }
        assert_eq!(bit(5, 15), 0, "outside");
        assert_eq!(bit(30, 15), 0, "the right edge is the source's half-open fill");
        assert!((t.vbuf[10 * 40 + 30 / 8] & (0x80 >> (30 & 7))) != 0, "the outline is in the vbuf");
    }

    #[test]
    fn a_box_off_the_top_clips_to_row_zero_closing_edge_first() {
        // The y pass moves the top corners to row 0; each pass starts with
        // the closing edge, so the x pass's first point is the y pass's last.
        let mut t = Techno::new();
        t.clipanypoly(&[(10, -10), (30, -10), (30, 10), (10, 10)]);
        assert_eq!(&t.polyxy[..t.polysides], &[(30, 10), (10, 10), (10, 0), (30, 0)]);
    }
}
