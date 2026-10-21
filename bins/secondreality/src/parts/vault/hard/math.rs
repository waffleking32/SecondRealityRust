//! HARD's rotation and projections: `rotlist`, `projlist` and `rotprojlist`
//! (`VEC.ASM:105-345`) under the profile `VDATA.ASM:30-36` sets.
//!
//! The rotation arithmetic is GLENZ's — three signed 16-bit matrix words
//! sign-extended and multiplied by the point's 32-bit coordinates, summed in
//! 64 bits and shifted right 15 (`shld ecx,ebx,17`) — but HARD stores the
//! axes in a different order. In `rotlist` the first sum (matrix words
//! 0/2/4) is pushed, the third (12/14/16) stays in `ecx`, `pop edx` takes
//! the second (6/8/10) and `pop eax` the first; the stores at
//! `VEC.ASM:172-176` then write x = third, y = second, z = first. GLENZ's
//! stores (`GLENZ/VEC.ASM:155-160`) put x = first, y = third, z = second, so
//! reusing its `rotate` rotates about different axes.
//!
//! `projlist` divides with a 32-bit `imul`/`idiv` by `max(z, 512)`, adds the
//! 160/64 centres to the quotient's low word with a 16-bit add, keeps the raw
//! x/y words at `+12`/`+14` for the clipper and sets the 0..100 window flags
//! (`VDATA.ASM:14-19`; only the dead `cpolylist` reads them). `rotprojlist`
//! instead multiplies only the coordinates' low words by the low word of the
//! projection constants (`imul word ptr`) and divides by the low word of z
//! (`idiv bx`) — a 16-bit path with no raw fields and no flags.

use crate::parts::glenz::math::Matrix;

/// `or bp,16` (`VEC.ASM:212`): z was below `projminz`, so the divide used the
/// clamp. (Only the dead `cpolylist` reads the flag word.)
pub const FLAG_Z: u16 = 16;
/// `or bp,8` (`VEC.ASM:220`): the projected y word is above `wmaxy`.
pub const FLAG_Y_MAX: u16 = 8;
/// `or bp,4` (`VEC.ASM:223`): the projected y word is below `wminy`.
pub const FLAG_Y_MIN: u16 = 4;
/// `or bp,2` (`VEC.ASM:233`): the projected x word is right of `wmaxx`.
pub const FLAG_X_MAX: u16 = 2;
/// `or bp,1` (`VEC.ASM:236`): the projected x word is left of `wminx`.
pub const FLAG_X_MIN: u16 = 1;

/// `wminx/wminy/wmaxx/wmaxy` (`VDATA.ASM:14-19`): the window `projlist`'s
/// flags compare against. It is HARD's, not `init320x200`'s 0..319/0..199;
/// the flags reach no live path.
const WINDOW_MIN: i16 = 0;
const WINDOW_MAX: i16 = 100;

/// HARD's projection profile (`VDATA.ASM:30-36`): `projxmul` 240,
/// `projymul` 200, `projxadd` 160, `projyadd` 64, `projminz` 512 — all dwords
/// in the source, except the two centre words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Projection {
    /// `projxmul`.
    pub mul_x: i32,
    /// `projymul`.
    pub mul_y: i32,
    /// `projxadd`, added to the quotient's low 16 bits.
    pub add_x: i16,
    /// `projyadd`, added to the quotient's low 16 bits.
    pub add_y: i16,
    /// `projminz`: the smallest z the divide uses.
    pub min_z: i32,
}

/// `projxmul=240, projymul=200, projxadd=160, projyadd=64, projminz=512`
/// (`VDATA.ASM:30-36`).
pub const HARD_PROJECTION: Projection = Projection {
    mul_x: 240,
    mul_y: 200,
    add_x: 160,
    add_y: 64,
    min_z: 512,
};

/// One record of HARD's `projlist` (`VEC.ASM:186-248`): the projected screen
/// x/y words at `+0`/`+2`, the flag word at `+4`, the unclamped z at `+8`,
/// and the raw x/y words at `+12`/`+14`.
///
/// x and y are the low 16 bits of the quotient plus the word centre, so they
/// may wrap; the flags compare the wrapped words. `raw_x`/`raw_y` are the
/// low words of the source coordinates, which `cclipeasypolylist` clips in
/// 2-D. `rotprojlist` writes neither the raw fields nor the flags.
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
    /// The raw x word stored at `+12` (`mov es:[di+12],cx`).
    pub raw_x: i16,
    /// The raw y word stored at `+14` (`mov es:[di+14],ax`).
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
/// translation.
///
/// The source appends to its destination list (`crotlist` adds the count at
/// `[di]`), but every caller resets the count first (`points2[0]=0;
/// crotlist(points2,cubepoints)`, `BP.C:273`), so `dst` is cleared.
///
/// The axis stores are HARD's, not GLENZ's: x uses matrix words 12/14/16, y
/// uses 6/8/10 and z uses 0/2/4 (`VEC.ASM:172-176`). With the identity
/// matrix the point `(1000,2000,3000)` becomes `(2999,1999,999)`, where
/// GLENZ's mapping gives `(999,2999,1999)`.
pub fn rotate(dst: &mut Vec<[i32; 3]>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3]) {
    dst.clear();
    dst.reserve(src.len());
    for &point in src {
        // The stores at `VEC.ASM:172-176`: x is the third sum (words
        // 12/14/16), y the second (6/8/10), z the first (0/2/4).
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

/// `projlist` (`VEC.ASM:186-248`): project `src` into `dst` with
/// [`HARD_PROJECTION`].
///
/// The order is y then x. Each axis is `(coordinate * multiplier) / clamped_z`
/// as a signed 64/32 `idiv` (truncating toward zero); the quotient's low 16
/// bits have the word centre added with a wrapping 16-bit `add`, and the
/// flags compare that word against the 0..100 window. The stored z is the
/// unclamped source z, and the raw x/y words are kept.
pub fn project(dst: &mut Vec<PVert>, src: &[[i32; 3]]) {
    dst.clear();
    dst.reserve(src.len());
    for &[x, y, z] in src {
        let mut flags = 0u16;
        let mut div_z = z;
        if div_z < HARD_PROJECTION.min_z {
            div_z = HARD_PROJECTION.min_z;
            flags |= FLAG_Z;
        }

        // `VEC.ASM:208-223`: y first. The source's `imul` forms a 64-bit
        // product and `idiv` divides it by the 32-bit z, truncating toward
        // zero; the quotient's low word takes the centre with a 16-bit add.
        let y_quotient =
            (i64::from(y) * i64::from(HARD_PROJECTION.mul_y) / i64::from(div_z)) as i32;
        let y16 = (y_quotient as i16).wrapping_add(HARD_PROJECTION.add_y);
        if y16 > WINDOW_MAX {
            flags |= FLAG_Y_MAX;
        }
        if y16 < WINDOW_MIN {
            flags |= FLAG_Y_MIN;
        }

        // `VEC.ASM:225-236`: then x.
        let x_quotient =
            (i64::from(x) * i64::from(HARD_PROJECTION.mul_x) / i64::from(div_z)) as i32;
        let x16 = (x_quotient as i16).wrapping_add(HARD_PROJECTION.add_x);
        if x16 > WINDOW_MAX {
            flags |= FLAG_X_MAX;
        }
        if x16 < WINDOW_MIN {
            flags |= FLAG_X_MIN;
        }

        dst.push(PVert {
            x: x16,
            y: y16,
            flags,
            // `mov dword ptr es:[di+8],ebx` (`VEC.ASM:208`): before the
            // clamp.
            z,
            // `mov es:[di+12],cx` (x, `VEC.ASM:226`) / `mov es:[di+14],ax`
            // (y, `VEC.ASM:214`): the raw coordinates' low words, for the
            // clipper.
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

/// `rotprojlist` (`VEC.ASM:250-345`): the same rotation as [`rotate`], then
/// the 16-bit projection [`project_word`] implements.
///
/// This is `crotprojlist`, the part-2 grid's path (`BP.C:182`). It differs
/// from [`project`] in every width: the coordinates' low words and the low
/// word of z are the operands (`imul word ptr gs:projymul`/`idiv bx`), and
/// the source's raw-field and z stores at `+12`/`+14` are commented out, so
/// those fields are zero here. No flags are set; only the dead `cpolylist`
/// reads them.
pub fn rotate_project(dst: &mut Vec<PVert>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3]) {
    dst.clear();
    dst.reserve(src.len());
    for &point in src {
        // The rotation is `rotlist`'s, stores and all (`VEC.ASM:276-322`).
        let x = rotate_axis(point, m, [6, 7, 8]).wrapping_add(add[0]);
        let y = rotate_axis(point, m, [3, 4, 5]).wrapping_add(add[1]);
        let z = rotate_axis(point, m, [0, 1, 2]).wrapping_add(add[2]);
        dst.push(PVert {
            // `VEC.ASM:326-335`: `imul word ptr gs:projymul` / `idiv bx` for
            // y, then `mov ax,word ptr gs:projxmul` / `imul cx` / `idiv bx`
            // for x. Both read the low words.
            x: project_word(
                x as i16,
                HARD_PROJECTION.mul_x as i16,
                HARD_PROJECTION.add_x,
                z as i16,
            ),
            y: project_word(
                y as i16,
                HARD_PROJECTION.mul_y as i16,
                HARD_PROJECTION.add_y,
                z as i16,
            ),
            // The raw-field stores (`VEC.ASM:325,331`) are commented out and
            // no flag word is written.
            flags: 0,
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

    /// `VDATA.ASM:30-36`'s profile, `projlist`'s two divides and the 0..100
    /// window flags.
    #[test]
    fn hard_projection_uses_240_200_160_64_and_min_z_512() {
        assert_eq!(
            HARD_PROJECTION,
            Projection {
                mul_x: 240,
                mul_y: 200,
                add_x: 160,
                add_y: 64,
                min_z: 512,
            }
        );

        let mut dst = Vec::new();
        project(&mut dst, &[[1000, 2000, 4000]]);
        assert_eq!(
            dst,
            vec![PVert {
                // 1000*240/4000 + 160 = 220; 2000*200/4000 + 64 = 164.
                x: 220,
                y: 164,
                // Both words are outside `wmaxx`/`wmaxy` = 100; z = 4000 is
                // not clamped.
                flags: FLAG_X_MAX | FLAG_Y_MAX,
                z: 4000,
                raw_x: 1000,
                raw_y: 2000,
            }]
        );

        // z = 100 is below `projminz`: the divide uses 512 (flag 16) and the
        // stored z stays 100. x = 1000*240/512 + 160 = 468 + 160 = 628;
        // y = -1000*200/512 + 64 = -390 + 64 = -326 (`idiv` truncates toward
        // zero).
        project(&mut dst, &[[1000, -1000, 100]]);
        assert_eq!(
            dst,
            vec![PVert {
                x: 628,
                y: -326,
                flags: FLAG_Z | FLAG_Y_MIN | FLAG_X_MAX,
                z: 100,
                raw_x: 1000,
                raw_y: -1000,
            }]
        );

        // The centre add is a 16-bit add: 100000*240/512 = 46875, whose word
        // plus 160 wraps to -18501 (and is left of the window).
        project(&mut dst, &[[100_000, 0, 512]]);
        assert_eq!(dst[0].x, 46_875i32.wrapping_add(160) as i16);
        assert_eq!(dst[0].flags, FLAG_X_MIN);
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
                flags: 0,
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
