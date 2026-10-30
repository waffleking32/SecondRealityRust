//! GLENZ's fixed-point vector maths: `MATH.ASM`'s matrix and `VEC.ASM`'s
//! rotate/project/clip list routines.
//!
//! The widths are the fidelity: the matrix is nine 16-bit words of `>> 15`
//! products, `rotlist` sums three 64-bit products and keeps 32 bits, and
//! `projlist` adds a word centre to the quotient's low 16 bits and compares
//! the word. A cleaner multiply, a wider compare or a truncating shift moves
//! every vertex. Angles are tenths of a degree, 0..3599.

use super::tables::{COS, SIN};

/// `or bp,16` (`VEC.ASM:198`): z was below the projection's `min_z`, so x
/// and y were projected through the clamped z.
pub const FLAG_Z: u16 = 16;
/// `or bp,8` (`VEC.ASM:205`): y is below the window (`y > max_y`).
pub const FLAG_Y_MAX: u16 = 8;
/// `or bp,4` (`VEC.ASM:208`): y is above the window (`y < min_y`).
pub const FLAG_Y_MIN: u16 = 4;
/// `or bp,2` (`VEC.ASM:217`): x is right of the window (`x > max_x`).
pub const FLAG_X_MAX: u16 = 2;
/// `or bp,1` (`VEC.ASM:220`): x is left of the window (`x < min_x`).
pub const FLAG_X_MIN: u16 = 1;

/// `calcmatrix`'s destination (`MATH.ASM:46-159`): nine signed words in the
/// source's word-offset order, `[0,2,4,6,8,10,12,14,16]`.
///
/// Word `i` is stored at byte offset `2*i`; the three groups are the three
/// sums `rotlist` forms, and the source consumes the third group (12/14/16)
/// as y and the second (6/8/10) as z (see [`rotate`]). In a right-handed
/// reading with the source's angles, the matrix is `(rZ·rX·rY)ᵀ`, not the
/// natural `rY·rX·rZ` row order; the code's y and z axes are the
/// mathematical z and y axes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matrix(pub [i16; 9]);

/// `checkdeg` (`MATH.ASM:17-28`): wrap an angle into `0..=3599`, the unsigned
/// `jb` for the range test, then the signed `jl` for the direction.
fn checkdeg(mut angle: i16) -> i16 {
    loop {
        if (angle as u16) < 3600 {
            return angle;
        }
        if angle < 3600 {
            angle = angle.wrapping_add(3600);
        } else {
            angle = angle.wrapping_sub(3600);
        }
    }
}

/// One `imul`/`shld dx,ax,1` pair: the signed 32-bit product shifted right 15
/// (arithmetically) and stored as a word (`MATH.ASM:88-96`).
fn mul15(a: i16, b: i16) -> i16 {
    ((i32::from(a) * i32::from(b)) >> 15) as i16
}

/// `cmatrix_yxz` (`MATH.ASM:30-44`) over `calcmatrix` (`MATH.ASM:46-159`):
/// build the matrix for three tenth-degree angles.
///
/// The C call is `cmatrix_yxz(rx, ry, rz, matrix)` (`MAIN.C:598`), but the asm
/// stores its *second* argument in the square it calls the X slot and its
/// first in the Y slot: `movpar ax,0` (arg0) goes to `[si+2]`, `movpar ax,1`
/// (arg1) to `[si+0]`, and arg2 to `[si+4]` (`MATH.ASM:36-41`). The labels in
/// `calcmatrix` are therefore shifted one place relative to the caller's
/// names, and this port keeps the source's placement: `a1` is the X angle,
/// `a0` the Y angle, `a2` the Z angle.
///
/// `m14` and `m12` shift a product of an already-shifted product, as the
/// source does, rather than forming the three-factor product once.
pub fn matrix_yxz(a0: i16, a1: i16, a2: i16) -> Matrix {
    // `calcmatrix`'s ROT-X, ROT-Y, ROT-Z (`MATH.ASM:52-78`); the angles in
    // the source's slots, not the caller's.
    let (rotx, roty, rotz) = (checkdeg(a1), checkdeg(a0), checkdeg(a2));
    let (sx, cx) = (SIN[rotx as usize], COS[rotx as usize]);
    let (sy, cy) = (SIN[roty as usize], COS[roty as usize]);
    let (sz, cz) = (SIN[rotz as usize], COS[rotz as usize]);

    // `MATH.ASM:88-156`, in the source's order. `m0a` is the first stored
    // product; `m14` subtracts a second product formed from it before `m0`
    // itself is reduced.
    let m0a = mul15(cy, cz);
    let mut m14 = mul15(sy, sz);
    m14 = m14.wrapping_sub(mul15(m0a, sx));

    let t = mul15(sx, sy);
    let m0 = m0a.wrapping_sub(mul15(sz, t));
    let mut m2 = mul15(cz, t);
    m2 = m2.wrapping_add(mul15(cy, sz));

    let mut m12 = mul15(sx, mul15(cy, sz));
    m12 = m12.wrapping_add(mul15(sy, cz));

    let m6 = mul15(cx, sz).wrapping_neg();
    let m8 = mul15(cx, cz);
    let m4 = mul15(cx, sy).wrapping_neg();
    let m16 = mul15(cx, cy);
    let m10 = sx;

    Matrix([m0, m2, m4, m6, m8, m10, m12, m14, m16])
}

/// One row of `rotlist`: three signed 16-bit matrix words sign-extended to 32
/// bits, multiplied by the point's 32-bit coordinates, summed in 64 bits and
/// shifted right 15 (`VEC.ASM:106-149`).
fn rotate_axis(point: [i32; 3], m: &Matrix, words: [usize; 3]) -> i32 {
    let sum = i64::from(m.0[words[0]]) * i64::from(point[0])
        + i64::from(m.0[words[1]]) * i64::from(point[1])
        + i64::from(m.0[words[2]]) * i64::from(point[2]);
    // `shld ecx,ebx,17`: the 64-bit pair shifted right 15, keeping the low 32
    // bits of the result. `as i32` truncates exactly as the register does.
    (sum >> 15) as i32
}

/// `rotlist` (`VEC.ASM:87-168`): rotate `src` into `dst` and add the
/// translation with a wrapping 32-bit `add`.
///
/// The source appends, but every shipped call resets the count first
/// (`MAIN.C:600`), so `dst` is cleared. The pops at `VEC.ASM:149-152` and the
/// stores at `:155-160` put words 0/2/4 in x, **12/14/16 in y** and 6/8/10 in
/// z: the source swaps the mathematical y and z axes.
pub fn rotate(dst: &mut Vec<[i32; 3]>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3]) {
    dst.clear();
    dst.reserve(src.len());
    for &point in src {
        let x = rotate_axis(point, m, [0, 1, 2]);
        let y = rotate_axis(point, m, [6, 7, 8]);
        let z = rotate_axis(point, m, [3, 4, 5]);
        dst.push([
            x.wrapping_add(add[0]),
            y.wrapping_add(add[1]),
            z.wrapping_add(add[2]),
        ]);
    }
}

/// `projlist`'s globals (`ADATA.ASM:18-39`, set by `init320x200`,
/// `VID.ASM:199-218`).
///
/// `add_x`/`add_y` are words in the source (`projxadd dw`, `projyadd dw`) and
/// are added to the *word* quotient with a 16-bit `add`; `min_z` and the two
/// multipliers are dwords.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projection {
    /// `projxmul`: the x scale.
    pub mul_x: i32,
    /// `projymul`: the y scale.
    pub mul_y: i32,
    /// `projxadd`: the centre's x, added to the low 16 bits.
    pub add_x: i16,
    /// `projyadd`: the centre's y, added to the low 16 bits.
    pub add_y: i16,
    /// `projminz`: the smallest z the divide uses. z below it is also flagged
    /// with [`FLAG_Z`], but the stored z stays the source's.
    pub min_z: i32,
    /// `wminx`: the window's left edge, inclusive.
    pub min_x: i16,
    /// `wmaxx`: the window's right edge, inclusive.
    pub max_x: i16,
    /// `wminy`: the window's top edge, inclusive.
    pub min_y: i16,
    /// `wmaxy`: the window's bottom edge, inclusive.
    pub max_y: i16,
}

/// `init320x200` (`VID.ASM:199-218`): the 320×200 mode's projection and
/// window, the values the shipped GLENZ path runs with.
pub const INIT_320X200: Projection = Projection {
    mul_x: 256,
    mul_y: 213,
    add_x: 160,
    add_y: 130,
    min_z: 128,
    min_x: 0,
    max_x: 319,
    min_y: 0,
    max_y: 199,
};

/// A projected point (`VEC.ASM:170-232`): the stored screen x/y words, the
/// flag word and the source's z.
///
/// x and y are the low 16 bits of the quotient plus the word centre, so they
/// may wrap; the flags are set from the wrapped words, not from the 32-bit
/// quotients. `z` is the *unclamped* source z (`mov dword ptr es:[di+8],ebx`
/// runs before the `projminz` test at `:194-198`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PVert {
    /// The projected x word.
    pub x: i16,
    /// The projected y word.
    pub y: i16,
    /// [`FLAG_Z`], [`FLAG_Y_MAX`], [`FLAG_Y_MIN`], [`FLAG_X_MAX`],
    /// [`FLAG_X_MIN`], or a combination.
    pub flags: u16,
    /// The source z, before the divide's clamp.
    pub z: i32,
}

/// `projlist` (`VEC.ASM:170-232`): project `src` into `dst` (cleared, as the
/// shipped caller resets the count, `MAIN.C:613`).
///
/// Each axis, y then x, is `(coordinate * multiplier) / z.max(min_z)` as a
/// truncating 64/32 `idiv`; the quotient's low word gets the word centre in a
/// wrapping 16-bit `add`, and the flags compare that word.
pub fn project(dst: &mut Vec<PVert>, src: &[[i32; 3]], p: &Projection) {
    dst.clear();
    dst.reserve(src.len());
    for &[x, y, z] in src {
        let mut flags = 0u16;
        let mut div_z = z;
        if div_z < p.min_z {
            div_z = p.min_z;
            flags |= FLAG_Z;
        }

        // `VEC.ASM:195-209`: y first.
        let y_quotient = ((i64::from(y) * i64::from(p.mul_y)) / i64::from(div_z)) as i32;
        let y16 = (y_quotient as i16).wrapping_add(p.add_y);
        if y16 > p.max_y {
            flags |= FLAG_Y_MAX;
        }
        if y16 < p.min_y {
            flags |= FLAG_Y_MIN;
        }

        // `VEC.ASM:211-221`: then x.
        let x_quotient = ((i64::from(x) * i64::from(p.mul_x)) / i64::from(div_z)) as i32;
        let x16 = (x_quotient as i16).wrapping_add(p.add_x);
        if x16 > p.max_x {
            flags |= FLAG_X_MAX;
        }
        if x16 < p.min_x {
            flags |= FLAG_X_MIN;
        }

        dst.push(PVert {
            x: x16,
            y: y16,
            flags,
            // `mov dword ptr es:[di+8],ebx` (`VEC.ASM:194`): before the clamp.
            z,
        });
    }
}

/// `cliplist` (`VEC.ASM:234-253`): clamp every point's y to `max`.
///
/// The source compares `ds:[si+4],1500` and stores 1500 when the value is not
/// less — `min(y, max)` — so y == max takes the store but is unchanged.
pub fn clip_y(points: &mut [[i32; 3]], max: i32) {
    for point in points {
        if point[1] >= max {
            point[1] = max;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pv(x: i16, y: i16, flags: u16, z: i32) -> PVert {
        PVert { x, y, flags, z }
    }

    fn project_one(point: [i32; 3], p: &Projection) -> PVert {
        let mut dst = Vec::new();
        project(&mut dst, &[point], p);
        dst[0]
    }

    /// A unit projection whose quotient is the coordinate, inside ±`edge`.
    fn unit(add_x: i16, edge: i16) -> Projection {
        Projection {
            mul_x: 1,
            mul_y: 1,
            add_x,
            add_y: 0,
            min_z: 1,
            min_x: -edge,
            max_x: edge,
            min_y: -edge,
            max_y: edge,
        }
    }

    /// The identity at (0,0,0) is `mul15(32767, 32767)` = 32766 on the
    /// diagonal, 0.99997.
    #[test]
    fn the_source_identity_at_zero_degrees_is_32766() {
        assert_eq!(matrix_yxz(0, 0, 0), Matrix([32766, 0, 0, 0, 32766, 0, 0, 0, 32766]));
    }

    /// The X slot (`a1`) at 90°: m0 = 32766, m14 = 0 - mul15(32766,32767) =
    /// -32765, m10 = sin900 = 32767.
    #[test]
    fn a_ninety_degree_x_angle_matches_the_hand_computed_words() {
        assert_eq!(matrix_yxz(0, 900, 0), Matrix([32766, 0, 0, 0, 0, 32767, 0, -32765, 0]));
    }

    /// All three angles, so `m14`'s use of the unreduced `m0` and `m12`'s
    /// double shift are pinned. X slot 700, Y slot 300, Z slot 1100; sin/cos
    /// (30791, 11207), (16383, 28377), (30791, -11206):
    /// m0 = mul15(28377,-11206) - mul15(30791, mul15(30791,16383)) = -9705 - 14465;
    /// m14 = mul15(16383,30791) - mul15(-9705,30791) = 15394 + 9120;
    /// m2 = mul15(-11206,15394) + mul15(28377,30791) = -5265 + 26664;
    /// m12 = mul15(30791, 26664) + mul15(16383,-11206) = 25055 - 5603.
    #[test]
    fn a_three_angle_matrix_matches_the_hand_computed_words() {
        assert_eq!(
            matrix_yxz(300, 700, 1100),
            Matrix([-24170, 21399, -5603, -10530, -3833, 30791, 19452, 24514, 9705])
        );
    }

    /// arg1 fills the X slot, arg0 the Y slot (`MATH.ASM:36-41`).
    #[test]
    fn matrix_yxz_puts_arg1_in_the_first_slot() {
        assert_eq!(matrix_yxz(900, 0, 0), Matrix([0, 0, -32766, 0, 32766, 0, 32766, 0, 0]));
    }

    /// `checkdeg`'s repeated ±3600, including the 16-bit ends.
    #[test]
    fn angles_wrap_by_3600() {
        let base = matrix_yxz(0, 123, 0);
        assert_eq!(matrix_yxz(0, 3600 + 123, 0), base);
        assert_eq!(matrix_yxz(0, -3600 + 123, 0), base);
        assert_eq!(matrix_yxz(0, 3 * 3600 + 123, 0), base);
        assert_eq!(matrix_yxz(0, -32768, 0), matrix_yxz(0, 3232, 0));
        assert_eq!(matrix_yxz(0, 32767, 0), matrix_yxz(0, 367, 0));
        assert_eq!(matrix_yxz(-1, 0, 0), matrix_yxz(3599, 0, 0));
        assert_eq!(matrix_yxz(3600, 0, 0), matrix_yxz(0, 0, 0));
        assert_eq!(matrix_yxz(0, 0, 7200), matrix_yxz(0, 0, 0));
    }

    /// The shift is arithmetic (`shld ecx,ebx,17`): -1 * 5000 floors to -1,
    /// and the identity's 32766/32768 takes 5000 to 4999.
    #[test]
    fn rotation_products_shift_down_not_toward_zero() {
        let mut dst = Vec::new();
        rotate(&mut dst, &[[5000, 0, 0]], &Matrix([-1, 0, 0, 0, 0, 0, 0, 0, 0]), [0, 0, 0]);
        assert_eq!(dst, vec![[-1, 0, 0]]);
        let identity = matrix_yxz(0, 0, 0);
        rotate(&mut dst, &[[5000, 0, 0]], &identity, [0, 0, 0]);
        assert_eq!(dst, vec![[4999, 0, 0]]);
        rotate(&mut dst, &[[32768, 0, 0]], &identity, [0, 0, 0]);
        assert_eq!(dst, vec![[32766, 0, 0]]);
    }

    /// The third sum (words 12/14/16) is y and the second (6/8/10) is z.
    #[test]
    fn rotate_takes_y_from_words_12_14_16_and_z_from_6_8_10() {
        let mut dst = Vec::new();
        rotate(&mut dst, &[[1000, 0, 0]], &Matrix([0, 0, 0, 0, 0, 0, 32766, 0, 0]), [0, 0, 0]);
        assert_eq!(dst, vec![[0, 999, 0]]);
        rotate(&mut dst, &[[1000, 0, 0]], &Matrix([0, 0, 0, 32766, 0, 0, 0, 0, 0]), [0, 0, 0]);
        assert_eq!(dst, vec![[0, 0, 999]]);
    }

    /// `rotate` clears the destination and adds the translation as a
    /// wrapping 32-bit add: `i32::MAX + 999` wraps.
    #[test]
    fn rotate_clears_extends_and_wraps_the_translation() {
        let m = Matrix([32767, 0, 0, 0, 32767, 0, 0, 0, 32767]);
        let mut dst = vec![[9, 9, 9], [8, 8, 8]];
        rotate(&mut dst, &[[1000, 0, 0], [0, 0, 0]], &m, [i32::MAX, 5, -3]);
        assert_eq!(dst, vec![[-2147482650, 5, -3], [i32::MAX, 5, -3]]);
    }

    /// The accumulator is 64 bits and the shifted sum keeps its low 32: the
    /// identity times `i32::MAX` is 2,147,352,575 (an i32 product would
    /// give -1); three 32767 * `i32::MAX` products shift to 6,442,254,333,
    /// stored as 2,147,287,037 (`VEC.ASM:109-121`).
    #[test]
    fn rotate_keeps_the_64_bit_product_and_its_low_32_bits() {
        let mut dst = Vec::new();
        rotate(&mut dst, &[[i32::MAX, 0, 0]], &matrix_yxz(0, 0, 0), [0, 0, 0]);
        assert_eq!(dst, vec![[2147352575, 0, 0]]);
        let m = Matrix([32767, 32767, 32767, 0, 0, 0, 0, 0, 0]);
        rotate(&mut dst, &[[i32::MAX, i32::MAX, i32::MAX]], &m, [0, 0, 0]);
        assert_eq!(dst, vec![[2147287037, 0, 0]]);
    }

    /// `idiv` truncates toward zero: -7*1000/3 is -2333.
    #[test]
    fn projection_truncates_toward_zero() {
        let p = Projection { mul_y: 1000, ..unit(0, 10_000) };
        assert_eq!(project_one([0, -7, 3], &p), pv(0, -2333, 0, 3));
    }

    /// The stored z is the unclamped one while the divide uses 128
    /// (`VEC.ASM:194-201`): z=64 projects x as 128*256/128+160 = 416.
    #[test]
    fn projection_stores_the_unclamped_z_and_divides_by_the_clamp() {
        let p = &INIT_320X200;
        assert_eq!(project_one([128, 0, 64], p), pv(416, 130, FLAG_Z | FLAG_X_MAX, 64));
        assert_eq!(project_one([0, 0, 100], p), pv(160, 130, FLAG_Z, 100));
    }

    /// Each flag at its boundary with the shipped projection at z = 128; the
    /// `jng`/`jnl` comparisons put equality inside.
    #[test]
    fn projection_flags_at_the_window_boundaries() {
        let p = &INIT_320X200;
        // x = 2*v + 160.
        assert_eq!(project_one([79, 0, 128], p), pv(318, 130, 0, 128));
        assert_eq!(project_one([80, 0, 128], p), pv(320, 130, FLAG_X_MAX, 128));
        assert_eq!(project_one([-80, 0, 128], p), pv(0, 130, 0, 128));
        assert_eq!(project_one([-81, 0, 128], p), pv(-2, 130, FLAG_X_MIN, 128));
        // y = 213*v/128 + 130, truncating.
        assert_eq!(project_one([0, 42, 128], p), pv(160, 199, 0, 128));
        assert_eq!(project_one([0, 43, 128], p), pv(160, 201, FLAG_Y_MAX, 128));
        assert_eq!(project_one([0, -78, 128], p), pv(160, 1, 0, 128));
        assert_eq!(project_one([0, -79, 128], p), pv(160, -1, FLAG_Y_MIN, 128));
        // z = 127 is below the clamp, z = 128 is not.
        assert_eq!(project_one([0, 0, 127], p), pv(160, 130, FLAG_Z, 127));
        assert_eq!(project_one([0, 0, 128], p), pv(160, 130, 0, 128));
        // A coordinate equal to an edge is inside; one past it is flagged.
        let p = unit(0, 50);
        assert_eq!(project_one([50, -50, 1], &p), pv(50, -50, 0, 1));
        assert_eq!(project_one([-50, 50, 1], &p), pv(-50, 50, 0, 1));
        assert_eq!(project_one([51, 0, 1], &p).flags, FLAG_X_MAX);
        assert_eq!(project_one([0, -51, 1], &p).flags, FLAG_Y_MIN);
    }

    /// The centre add and the comparisons are 16-bit: 20000*256/128 = 40000
    /// stores as -25536, +160 is -25376, and the word is left of 0.
    #[test]
    fn projection_compares_the_wrapped_word() {
        assert_eq!(
            project_one([20000, 0, 128], &INIT_320X200),
            pv(-25376, 130, FLAG_X_MIN, 128)
        );
        // 100 + 32767 = 32867, stored as -32669.
        assert_eq!(project_one([100, 0, 1], &unit(32767, 100)), pv(-32669, 0, FLAG_X_MIN, 1));
    }

    /// `project` clears its destination and handles every source point.
    #[test]
    fn projection_clears_the_destination() {
        let mut dst = vec![pv(1, 2, 3, 4)];
        project(&mut dst, &[[0, 0, 128], [0, 0, 128]], &INIT_320X200);
        assert_eq!(dst, vec![pv(160, 130, 0, 128); 2]);
    }

    /// `cliplist` clamps y to the limit and takes the `>=` boundary unchanged.
    #[test]
    fn clip_y_clamps_at_the_limit() {
        let mut points = [[0, 1499, 1], [2, 1500, 3], [4, 1501, 5], [6, -20000, 7]];
        clip_y(&mut points, 1500);
        assert_eq!(points, [[0, 1499, 1], [2, 1500, 3], [4, 1500, 5], [6, -20000, 7]]);
    }
}
