//! HARD's rotation and projections: `rotlist`, `projlist` and `rotprojlist`
//! (`VEC.ASM:105-345`) under the profile `VDATA.ASM:30-36` sets.
//!
//! The rotation arithmetic is GLENZ's (16-bit matrix words times 32-bit
//! coordinates, summed in 64 bits, `shld ecx,ebx,17`), but the stores at
//! `VEC.ASM:172-176` write x = the third sum, y = the second, z = the first,
//! where GLENZ's (`GLENZ/VEC.ASM:155-160`) write x = first, y = third, z =
//! second. `projlist` divides 32-bit by `max(z, 512)` and keeps the raw x/y
//! words for the clipper; its window flags (`VDATA.ASM:14-19`) are read only
//! by the dead `cpolylist` and are not ported. `rotprojlist` projects in 16
//! bits: low words times low words, divided by z's low word.

use crate::parts::glenz::math::Matrix;

/// `projxmul`, `projymul`, `projxadd`, `projyadd`, `projminz`
/// (`VDATA.ASM:30-36`). The centres are words, added to the quotient's low
/// 16 bits.
pub const MUL_X: i32 = 240;
pub const MUL_Y: i32 = 200;
pub const ADD_X: i16 = 160;
pub const ADD_Y: i16 = 64;
const MIN_Z: i32 = 512;

/// One projected point: `projlist`'s screen x/y words at `+0`/`+2`, the
/// unclamped z at `+8` and the raw x/y words at `+12`/`+14` (`VEC.ASM:186-248`).
/// x and y may wrap. `rotprojlist` writes no raw fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PVert {
    pub x: i16,
    pub y: i16,
    /// The source z, before the divide's clamp.
    pub z: i32,
    /// `mov es:[di+12],cx`.
    pub raw_x: i16,
    /// `mov es:[di+14],ax`.
    pub raw_y: i16,
}

/// One `rotlist` axis: three signed matrix words, sign-extended to 32 bits,
/// times the point's 32-bit coordinates, summed in 64 bits and shifted right
/// 15 (`shld ecx,ebx,17`, `VEC.ASM:137`).
fn rotate_axis(point: [i32; 3], m: &Matrix, words: [usize; 3]) -> i32 {
    let sum = i64::from(m.0[words[0]]) * i64::from(point[0])
        + i64::from(m.0[words[1]]) * i64::from(point[1])
        + i64::from(m.0[words[2]]) * i64::from(point[2]);
    (sum >> 15) as i32
}

/// `rotlist` (`VEC.ASM:105-184`): rotate `src` into `dst` and add HARD's
/// translation. Every caller resets the destination's count first
/// (`BP.C:273`), so `dst` is cleared.
pub fn rotate(dst: &mut Vec<[i32; 3]>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3]) {
    dst.clear();
    dst.reserve(src.len());
    for &point in src {
        let x = rotate_axis(point, m, [6, 7, 8]);
        let y = rotate_axis(point, m, [3, 4, 5]);
        let z = rotate_axis(point, m, [0, 1, 2]);
        dst.push([
            x.wrapping_add(add[0]),
            y.wrapping_add(add[1]),
            z.wrapping_add(add[2]),
        ]);
    }
}

/// `projlist` (`VEC.ASM:186-248`): y then x, each `(coordinate * mul) /
/// max(z, 512)` as a signed 64/32 `idiv`, the quotient's low word plus the
/// centre in a wrapping 16-bit add.
pub fn project(dst: &mut Vec<PVert>, src: &[[i32; 3]]) {
    dst.clear();
    dst.reserve(src.len());
    for &[x, y, z] in src {
        let div_z = i64::from(z.max(MIN_Z));
        let y_quotient = (i64::from(y) * i64::from(MUL_Y) / div_z) as i32;
        let x_quotient = (i64::from(x) * i64::from(MUL_X) / div_z) as i32;
        dst.push(PVert {
            x: (x_quotient as i16).wrapping_add(ADD_X),
            y: (y_quotient as i16).wrapping_add(ADD_Y),
            // `mov dword ptr es:[di+8],ebx` (`VEC.ASM:208`): before the clamp.
            z,
            raw_x: x as i16,
            raw_y: y as i16,
        });
    }
}

/// One `imul word ptr`/`idiv bx` pair from `rotprojlist`
/// (`VEC.ASM:326-335`): the coordinate's **low word** times the constant's
/// low word gives a 32-bit `DX:AX` product, which `idiv` divides by the low
/// word of z; the quotient's low word takes the centre with a 16-bit add.
fn project_word(coordinate: i16, multiplier: i16, add: i16, z: i16) -> i16 {
    // `imul word ptr`: `DX:AX = AX * operand`, the full signed 32-bit
    // 16x16 product.
    let product = i32::from(coordinate) * i32::from(multiplier);
    // `idiv bx`: the signed 32-by-16 division, quotient's low word. A zero
    // divisor (or an overflowing quotient) faults in the source; the
    // shipped geometry reaches neither, and the guard keeps the port from
    // panicking where the source would stop.
    let quotient = product.checked_div(i32::from(z)).unwrap_or(0);
    (quotient as i16).wrapping_add(add)
}

/// `rotprojlist` (`VEC.ASM:250-345`), `crotprojlist` for the part-2 grid
/// (`BP.C:182`): [`rotate`]'s rotation, then [`project_word`] on the low
/// words; the raw-field stores (`VEC.ASM:325,331`) are commented out.
pub fn rotate_project(dst: &mut Vec<PVert>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3]) {
    dst.clear();
    dst.reserve(src.len());
    for &point in src {
        let x = rotate_axis(point, m, [6, 7, 8]).wrapping_add(add[0]);
        let y = rotate_axis(point, m, [3, 4, 5]).wrapping_add(add[1]);
        let z = rotate_axis(point, m, [0, 1, 2]).wrapping_add(add[2]);
        dst.push(PVert {
            x: project_word(x as i16, MUL_X as i16, ADD_X, z as i16),
            y: project_word(y as i16, MUL_Y as i16, ADD_Y, z as i16),
            z,
            raw_x: 0,
            raw_y: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The source's axis stores, against the same matrix and point through
    /// GLENZ's rotation. HARD's identity-matrix mapping takes x from the
    /// words 12/14/16 (point z), y from 6/8/10 (point y) and z from 0/2/4
    /// (point x), so `(1000,2000,3000)` is `(2999,1999,999)`; GLENZ's is
    /// `(999,2999,1999)`. The translation adds on the same permuted axes.
    #[test]
    fn hard_rotation_takes_x_from_words_12_14_16_and_z_from_0_2_4() {
        let identity = Matrix([32767, 0, 0, 0, 32767, 0, 0, 0, 32767]);
        let mut dst = Vec::new();
        rotate(&mut dst, &[[1000, 2000, 3000]], &identity, [0, 0, 0]);
        assert_eq!(dst, vec![[2999, 1999, 999]]);

        let mut glenz = Vec::new();
        crate::parts::glenz::math::rotate(&mut glenz, &[[1000, 2000, 3000]], &identity, [0, 0, 0]);
        assert_eq!(glenz, vec![[999, 2999, 1999]]);
        assert_ne!(dst, glenz);

        rotate(&mut dst, &[[1000, 2000, 3000]], &identity, [10, -20, 30]);
        assert_eq!(dst, vec![[3009, 1979, 1029]]);
    }

    /// `VDATA.ASM:30-36`'s profile and `projlist`'s two divides.
    #[test]
    fn hard_projection_uses_240_200_160_64_and_min_z_512() {
        let mut dst = Vec::new();
        project(&mut dst, &[[1000, 2000, 4000]]);
        // 1000*240/4000 + 160 = 220; 2000*200/4000 + 64 = 164.
        assert_eq!(dst, vec![PVert { x: 220, y: 164, z: 4000, raw_x: 1000, raw_y: 2000 }]);

        // z = 100 divides by 512 and is stored as 100: 1000*240/512 + 160 =
        // 628, -1000*200/512 + 64 = -326 (`idiv` truncates toward zero).
        project(&mut dst, &[[1000, -1000, 100]]);
        assert_eq!(dst, vec![PVert { x: 628, y: -326, z: 100, raw_x: 1000, raw_y: -1000 }]);

        // The centre add is 16-bit: 100000*240/512 = 46875, plus 160, wraps.
        project(&mut dst, &[[100_000, 0, 512]]);
        assert_eq!(dst[0].x, 46_875i32.wrapping_add(160) as i16);
        assert_eq!((dst[0].raw_x, dst[0].raw_y), (100_000i32 as i16, 0));
    }

    /// `crotprojlist`'s 16-bit widths: only the low words of the coordinates
    /// and of z are operands, while `imul word ptr` still forms the full
    /// 32-bit `DX:AX` product. The 32-bit `projlist` on the same rotated
    /// point divides by the full z and gives a different screen pair.
    #[test]
    fn rotate_projection_is_the_16_bit_crotprojlist() {
        let identity = Matrix([32767, 0, 0, 0, 32767, 0, 0, 0, 32767]);
        let mut dst = Vec::new();
        // HARD's mapping rotates (200,200,300) to x = 299 (from point z),
        // y = 199 (point y), z = 199 (point x); add_z lifts z to 65735,
        // whose low word is 199.
        rotate_project(&mut dst, &[[200, 200, 300]], &identity, [0, 0, 65536]);
        assert_eq!(
            dst,
            vec![PVert {
                // 299*240 = 71760 and 199*200 = 39800 overflow a signed word;
                // `imul word ptr` keeps the 32-bit product, and `idiv bx`
                // divides by 199: 71760/199 + 160 = 360 + 160 = 520,
                // 39800/199 + 64 = 200 + 64 = 264. (A 16-bit-wrapped product
                // would give 6224/199 + 160 = 191 and -25736/199 + 64 = -65.)
                x: 520,
                y: 264,
                z: 65_735,
                raw_x: 0,
                raw_y: 0,
            }]
        );

        // The 32-bit `projlist` divides by 65735 instead: (161, 64), with the
        // raw fields written.
        let mut wide = Vec::new();
        project(&mut wide, &[[299, 199, 65_735]]);
        assert_eq!((wide[0].x, wide[0].y), (161, 64));
        assert_eq!((wide[0].raw_x, wide[0].raw_y), (299, 199));
        assert_ne!((dst[0].x, dst[0].y), (wide[0].x, wide[0].y));
    }
}
