//! The vector engine's fixed-point maths, transliterated from `VISU/ACALC.ASM`.
//!
//! Three multiply paths exist in the original and are deliberately kept
//! apart: `mulmatrices` (`ACALC.ASM:67-143`) keeps each 32-bit product's low
//! half in `eax`, accumulates in 32 bits and ends with `sar 14`;
//! `rotatesingle` (`:415-468`) accumulates in `ecx:ebx` and ends with
//! `shrd ebx,ecx,14`, keeping bits 14..45 of the full 64-bit sum; and
//! `calc_rotate`/`calc_nrotate` (`:512-681`) read the matrix entries with
//! `movsx` as signed 16-bit words before using the same 64-bit `shrd`, while
//! `calc_singlez` (`:585-612`) reads the three matrix entries as full dwords.
//!
//! The projected-vertex flags and clips are `AD.INC`'s `VF_*`; the projection
//! variables are `ADATA.ASM`'s `_proj*` and the window is `AVID.ASM:92-115`.
//! The camera angle uses the 256-word `256/tan` table from `AVISTAN.INC`.
//! The local vertex and normal are `sr_assets::visu`'s `vlist`/`nlist`
//! records, read as they are stored.

use sr_assets::visu::{Normal, Vertex};

/// The fixed-point unit, `unit equ 16384` (`AD.INC:41`).
#[cfg(test)]
pub const UNIT: i32 = 16384;

/// The fixed-point shift, `unitshr equ 14` (`AD.INC:42`).
pub const UNITSHR: u32 = 14;

/// The vertex is above the window (`AD.INC:44`).
pub const VF_UP: u16 = 1;
/// The vertex is below the window (`AD.INC:45`).
pub const VF_DOWN: u16 = 2;
/// The vertex is left of the window (`AD.INC:46`).
pub const VF_LEFT: u16 = 4;
/// The vertex is right of the window (`AD.INC:47`).
pub const VF_RIGHT: u16 = 8;
/// The vertex's z was below `clip_z[0]`, so `x`/`y` are undefined
/// (`AD.INC:48`, `CD.H:27`).
pub const VF_NEAR: u16 = 16;
/// The vertex's z was above `clip_z[1]` (`AD.INC:49`). `CD.H` does not name
/// it, but `calc_project` sets it (`ACALC.ASM:779-781`).
pub const VF_FAR: u16 = 32;

/// The shipped `vid_window`'s half-width, `clip_x[1] - add_x` = 319 - 159,
/// which `_vid_cameraangle` starts from (`AVID.ASM:60-63`). Both parts install
/// that window (`U2A.C:247`, `U2E.C:351`); another window would need it
/// computed from the [`Projection`].
const HALF_WIDTH: i32 = 160;

/// `_avistan`: `AVISTAN.INC`'s 256 words (lines 2-23, "256 words = 90
/// degrees"), verbatim. Entry `i` is `256/tan(i * 90/256°)`, saturating at
/// 32767 for the two zero-angle entries.
#[rustfmt::skip]
pub const AVISTAN: [u16; 256] = [
    32767, 32767, 20859, 13905, 10428, 8341, 6950, 5956, 5210, 4631, 4166, 3787,
    3470, 3202, 2972, 2773, 2599, 2445, 2308, 2185, 2075, 1975, 1884, 1801,
    1725, 1655, 1591, 1531, 1475, 1423, 1374, 1329, 1286, 1246, 1209, 1173,
    1140, 1108, 1077, 1049, 1022, 996, 971, 947, 925, 903, 882, 862,
    843, 825, 808, 791, 774, 759, 744, 729, 715, 701, 688, 675,
    663, 651, 640, 628, 618, 607, 597, 587, 577, 568, 558, 549,
    541, 532, 524, 516, 508, 500, 493, 486, 478, 471, 465, 458,
    451, 445, 439, 433, 427, 421, 415, 409, 404, 398, 393, 388,
    383, 378, 373, 368, 363, 358, 354, 349, 345, 340, 336, 332,
    328, 323, 319, 315, 311, 308, 304, 300, 296, 293, 289, 285,
    282, 278, 275, 272, 268, 265, 262, 259, 256, 252, 249, 246,
    243, 240, 237, 234, 232, 229, 226, 223, 220, 218, 215, 212,
    210, 207, 204, 202, 199, 197, 194, 192, 189, 187, 185, 182,
    180, 177, 175, 173, 171, 168, 166, 164, 162, 159, 157, 155,
    153, 151, 149, 147, 145, 142, 140, 138, 136, 134, 132, 130,
    128, 126, 124, 123, 121, 119, 117, 115, 113, 111, 109, 107,
    106, 104, 102, 100, 98, 96, 95, 93, 91, 89, 88, 86,
    84, 82, 81, 79, 77, 75, 74, 72, 70, 69, 67, 65,
    64, 62, 60, 59, 57, 55, 54, 52, 50, 49, 47, 46,
    44, 42, 41, 39, 37, 36, 34, 33, 31, 29, 28, 26,
    25, 23, 22, 20, 18, 17, 15, 14, 12, 11, 9, 7,
    6, 4, 3, 1,
];

/// The original's `rmatrix` (`CD.H:29-36`, `AD.INC:35-39`): the row-major
/// 3x3 rotation, then the position.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Matrix {
    pub m: [i32; 9],
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[cfg(test)]
impl Matrix {
    /// The unit rotation at the origin.
    pub fn identity() -> Matrix {
        Matrix { m: [UNIT, 0, 0, 0, UNIT, 0, 0, 0, UNIT], ..Default::default() }
    }
}

/// A vertex after `calc_rotate` (`vlist`, `CD.H:38-46`): full dwords, and the
/// normal index copied unchanged.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Rotated {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub normal: i16,
}

/// A normal after `calc_nrotate` (`nlist`, `CD.H:48-55`): 16-bit words.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct RotatedNormal {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

/// A projected vertex (`pvlist`, `CD.H:57-64`): the 16-bit words
/// `calc_project` stores and the `VF_*` flags. Under `VF_NEAR`, `x` and `y`
/// are undefined (`CD.H:61`).
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct PVert {
    pub x: i16,
    pub y: i16,
    pub vf: u16,
}

/// The globals `_proj*` (`ADATA.ASM:32-40`) that drive `calc_project` and
/// `_vid_cameraangle`.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct Projection {
    /// `_projmulx`, the x scale.
    pub mul_x: i32,
    /// `_projmuly`, the y scale.
    pub mul_y: i32,
    /// `_projaddx`, the window's centre x.
    pub add_x: i32,
    /// `_projaddy`, the window's centre y.
    pub add_y: i32,
    /// `_projclipx` as `[min, max]`.
    pub clip_x: [i32; 2],
    /// `_projclipy` as `[min, max]`.
    pub clip_y: [i32; 2],
    /// `_projclipz` as `[min, max]`.
    pub clip_z: [i32; 2],
}

/// `_calc_applyrmatrix` (`ACALC.ASM:388-413`): `dest = apply x dest` (the
/// asm's order), then rotate `dest`'s position by `apply` and add `apply`'s
/// position.
///
/// The matrix product is `mulmatrices2`/`mulmatrices` (`:67-143`): each term
/// is `imul`'s low 32 bits, accumulated in 32 bits, then `sar 14`. The
/// position rotation is `rotatesingle` (`:415-468`): full 64-bit products,
/// `shrd 14`, then the 32-bit translation.
pub fn apply_matrix(dest: &mut Matrix, apply: &Matrix) {
    let mut m = [0i32; 9];
    for row in 0..3 {
        for col in 0..3 {
            let mut acc: i32 = 0;
            for k in 0..3 {
                // `imul` leaves the low 32 bits in eax; `add ebx,eax` wraps.
                acc = acc.wrapping_add(apply.m[row * 3 + k].wrapping_mul(dest.m[k * 3 + col]));
            }
            // `sar ebx,unitshr`.
            m[row * 3 + col] = acc >> UNITSHR;
        }
    }
    let rotated = rotate_position(dest.x, dest.y, dest.z, apply);
    dest.m = m;
    // `add fs:[si+rmatrix_x],eax` (`ACALC.ASM:406-412`): the translation,
    // after the rotation.
    dest.x = rotated.0.wrapping_add(apply.x);
    dest.y = rotated.1.wrapping_add(apply.y);
    dest.z = rotated.2.wrapping_add(apply.z);
}

/// `rotatesingle` (`ACALC.ASM:415-468`) with the position as its source:
/// `ecx:ebx` accumulates the full 64-bit products and `shrd ebx,ecx,14`
/// keeps bits 14..45. The matrix entries are dwords, not signed words.
fn rotate_position(x: i32, y: i32, z: i32, m: &Matrix) -> (i32, i32, i32) {
    let src = [x, y, z];
    let mut out = [0i32; 3];
    for (row, slot) in out.iter_mut().enumerate() {
        let mut sum: i64 = 0;
        for (k, &v) in src.iter().enumerate() {
            // `add ebx,eax` / `adc ecx,edx`: a 64-bit two's-complement sum.
            sum = sum.wrapping_add((m.m[row * 3 + k] as i64).wrapping_mul(v as i64));
        }
        *slot = (sum >> UNITSHR) as i32;
    }
    (out[0], out[1], out[2])
}

/// `_calc_rotate` (`ACALC.ASM:512-578`): rotate `verts` into `out` with `m`
/// and add its position. The matrix entries are read with `movsx` as signed
/// 16-bit words; the products are full 64-bit, `shrd 14`; the position add
/// and the stores are 32-bit. The vertex's normal index is copied unchanged.
///
/// Processes `min(out.len(), verts.len())` vertices.
pub fn rotate_vertices(out: &mut [Rotated], verts: &[Vertex], m: &Matrix) {
    // `movsx eax, word ptr ds:[si+rmatrix_m+2*n]`: the entries are signed
    // words here, whatever the Matrix holds above bit 15.
    let entry = |i: usize| m.m[i] as i16 as i32;
    let r0 = [entry(0), entry(1), entry(2)];
    let r1 = [entry(3), entry(4), entry(5)];
    let r2 = [entry(6), entry(7), entry(8)];
    for (dst, src) in out.iter_mut().zip(verts) {
        *dst = Rotated {
            x: rotate_axis(src, &r0, m.x),
            y: rotate_axis(src, &r1, m.y),
            z: rotate_axis(src, &r2, m.z),
            normal: src.normal,
        };
    }
}

/// One row of `calc_rotate`: three 64-bit `imul` products accumulated in
/// `ecx:ebx`, `shrd 14`, then the position added as a 32-bit dword.
fn rotate_axis(v: &Vertex, row: &[i32; 3], translate: i32) -> i32 {
    let mut sum: i64 = 0;
    for (a, p) in row.iter().zip([v.x, v.y, v.z]) {
        sum = sum.wrapping_add((*a as i64).wrapping_mul(p as i64));
    }
    ((sum >> UNITSHR) as i32).wrapping_add(translate)
}

/// `_calc_nrotate` (`ACALC.ASM:621-681`): rotate the 16-bit normals with
/// `m`'s rotation. The matrix entries and the normals are signed 16-bit
/// words; the products accumulate in 32 bits and `shrd 14` leaves the low 16
/// bits, stored as words. The position is not used.
pub fn rotate_normals(out: &mut [RotatedNormal], normals: &[Normal], m: &Matrix) {
    let entry = |i: usize| m.m[i] as i16 as i32;
    let r0 = [entry(0), entry(1), entry(2)];
    let r1 = [entry(3), entry(4), entry(5)];
    let r2 = [entry(6), entry(7), entry(8)];
    for (dst, src) in out.iter_mut().zip(normals) {
        *dst = RotatedNormal {
            x: rotate_normal_axis(src, &r0),
            y: rotate_normal_axis(src, &r1),
            z: rotate_normal_axis(src, &r2),
        };
    }
}

/// One row of `calc_nrotate` (`ACALC.ASM:630-673`): `imul word ptr` products
/// of two signed 16-bit words, accumulated in `bx:cx` (32 bits), `shrd 14`,
/// and the low 16 bits stored as a word.
fn rotate_normal_axis(v: &Normal, row: &[i32; 3]) -> i16 {
    let mut acc: i32 = 0;
    for (a, p) in row.iter().zip([v.x, v.y, v.z]) {
        acc = acc.wrapping_add((*a).wrapping_mul(p as i32));
    }
    (acc >> UNITSHR) as i16
}

/// `_vid_window` (`AVID.ASM:92-115`): set the clip window and its centre.
/// `add_x` and `add_y` are `(min + max) sar 1` — an arithmetic shift, so a
/// negative sum rounds down.
pub fn window(x1: i32, x2: i32, y1: i32, y2: i32, z1: i32, z2: i32) -> Projection {
    // `mov edx,eax / add edx,eax / sar edx,1` (`AVID.ASM:94-99`): a 32-bit
    // sum shifted arithmetically. The multipliers are not touched here; the
    // original leaves the globals as they were and `camera_angle` sets them.
    Projection {
        mul_x: 0,
        mul_y: 0,
        add_x: x1.wrapping_add(x2) >> 1,
        add_y: y1.wrapping_add(y2) >> 1,
        clip_x: [x1, x2],
        clip_y: [y1, y2],
        clip_z: [z1, z2],
    }
}

/// `_vid_cameraangle` (`AVID.ASM:60-85`): `mul_x` from the `256/tan` table,
/// then `mul_y` from the aspect (a 16-bit word in the original,
/// `ADATA.ASM:40`).
///
/// `angle` is the part's full field-of-view angle — the animation stream's
/// `fov` byte shifted up by 8 (`fov = a << 8`, `U2A.C:348`, `U2E.C:457`).
/// The asm halves it (`shr bx,1`, `AVID.ASM:65`), clamps the half-angle to
/// `8*64 ..= 16383` (`:68-76`), and indexes `_avistan` with
/// `(bx >> 5) & !1` as a byte offset into the word table — word index
/// `half >> 6` (`:77-80`) — before the unsigned `mul`/`shrd 8` and the
/// aspect multiply. The shipped pairs are `(0x2200, 225) -> (360, 316)` for
/// U2A and `(0x1C00, 225) -> (446, 391)` for U2E.
pub fn camera_angle(angle: u16, aspect: i32) -> (i32, i32) {
    // `shr bx,1 ;divide by 2 for half of angle` (`AVID.ASM:65`) and the
    // `cmp bx,8*64` / `cmp bx,16384` clamps (`:68-76`).
    let half = i32::from(angle >> 1).clamp(8 * 64, 16383);
    // `shr bx,6-1 / and bx,not 1` then a word read: the table index is
    // `half >> 6` (`AVID.ASM:77-80`).
    let tan = AVISTAN[(half >> 6) as usize];
    // `mul ecx / shrd eax,edx,8`: an unsigned 32-bit multiply by the 16-bit
    // table word, shifted right by 8.
    let mul_x = ((HALF_WIDTH as u32 * u32::from(tan)) >> 8) as i32;
    // `movzx ecx,word ptr _projaspect / mul ecx / shrd eax,edx,8`.
    let aspect = i32::from(aspect as u16);
    let mul_y = ((mul_x as u32 * aspect as u32) >> 8) as i32;
    (mul_x, mul_y)
}

/// `calc_project`'s body (`ACALC.ASM:759-817`): clip z (setting `VF_NEAR`
/// and substituting `clip_z[0]`, or flagging `VF_FAR`), project y, then x,
/// with truncating `idiv`, store the low 16 bits of each, and set the
/// up/down/left/right flags from the full 32-bit results.
///
/// `calc_project` reads a 32-bit `vlist` source and writes a separate
/// `pvlist`; `src` is the rotated vertex and `out` the projected one (a
/// `pvlist` has no `z`, so the source cannot be the projected vertex).
pub fn project(src: &Rotated, out: &mut PVert, p: &Projection) {
    let mut z = src.z;
    let mut vf = 0u16;
    // The z clip comes first and substitutes its limit before the divides
    // (`ACALC.ASM:774-782`).
    if z < p.clip_z[0] {
        vf |= VF_NEAR;
        z = p.clip_z[0];
    } else if z > p.clip_z[1] {
        vf |= VF_FAR;
    }

    // y first (`:783-793`), then x (`:795-804`).
    let y = projected(src.y, p.mul_y, z).wrapping_add(p.add_y);
    if y > p.clip_y[1] {
        vf |= VF_DOWN;
    }
    if y < p.clip_y[0] {
        vf |= VF_UP;
    }
    out.y = y as i16;

    let x = projected(src.x, p.mul_x, z).wrapping_add(p.add_x);
    if x > p.clip_x[1] {
        vf |= VF_RIGHT;
    }
    if x < p.clip_x[0] {
        vf |= VF_LEFT;
    }
    out.x = x as i16;
    out.vf = vf;
}

/// `imul` into `edx:eax` followed by `idiv` (`ACALC.ASM:783-784`): a signed
/// 64-bit product divided by the 32-bit z, truncating toward zero.
fn projected(v: i32, mul: i32, z: i32) -> i32 {
    ((v as i64 * mul as i64) / z as i64) as i32
}

/// `calc_project` over a list (`ACALC.ASM:759-817`): project every rotated
/// vertex into `out` and return the `and ax,bp` of their flags, starting
/// from `0xFFFF` (`:810-814`). An empty list returns `0xFFFF`; the original
/// skips its loop for a zero count. Processes `min` of the two lengths.
pub fn project_all(rotated: &[Rotated], out: &mut [PVert], p: &Projection) -> u16 {
    let mut vf = 0xFFFF;
    for (src, dst) in rotated.iter().zip(out.iter_mut()) {
        project(src, dst, p);
        vf &= dst.vf;
    }
    vf
}

/// `_calc_singlez` (`ACALC.ASM:585-612`): the transformed z of one *local*
/// vertex, used for the object sort. The three matrix entries are read as
/// full dwords (`mov eax`), unlike `calc_rotate`'s `movsx`; the product is
/// 64-bit, `shrd 14`, and the position's z is added in 32 bits.
pub fn single_z(verts: &[Vertex], index: u16, m: &Matrix) -> i32 {
    let v = &verts[index as usize];
    // `mov eax,ds:[si+rmatrix_m+24]` — a dword, unlike `calc_rotate`.
    let mut sum: i64 = 0;
    for (a, p) in [m.m[6], m.m[7], m.m[8]].iter().zip([v.x, v.y, v.z]) {
        sum = sum.wrapping_add((*a as i64).wrapping_mul(p as i64));
    }
    ((sum >> UNITSHR) as i32).wrapping_add(m.z)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The window both parts install: `vid_window(0,319,25,174,512,9999999)`
    /// (`U2A.C:247`, `U2E.C:351`).
    fn shipped_window() -> Projection {
        window(0, 319, 25, 174, 512, 9_999_999)
    }

    fn vertex(x: i32, y: i32, z: i32) -> Vertex {
        Vertex { x, y, z, normal: 0 }
    }

    fn rotated(x: i32, y: i32, z: i32) -> Rotated {
        Rotated { x, y, z, normal: 0 }
    }

    #[test]
    fn window_sets_the_shipped_centre_and_clips() {
        let p = shipped_window();
        assert_eq!(p.add_x, 159);
        assert_eq!(p.add_y, 99);
        assert_eq!(p.clip_x, [0, 319]);
        assert_eq!(p.clip_y, [25, 174]);
        assert_eq!(p.clip_z, [512, 9_999_999]);
    }

    #[test]
    fn window_shifts_the_sum_arithmetically() {
        // `sar edx,1` rounds a negative centre down, not toward zero
        // (`AVID.ASM:96-98`).
        let p = window(-1, 0, 0, 1, 0, 0);
        assert_eq!(p.add_x, -1);
        assert_eq!(p.add_y, 0);
    }

    #[test]
    fn camera_angle_matches_the_two_shipped_pairs() {
        // The stream bytes are the full angles: U2A's 0x22 -> 0x2200
        // (47.8 degrees) and U2E's 0x1C -> 0x1C00 (39.4 degrees), both with
        // aspect 225. `_vid_cameraangle` halves the angle before indexing
        // (`shr bx,1`, `AVID.ASM:65`), so 0x2200 selects table word 68 (577)
        // and 0x1C00 selects word 56 (715).
        assert_eq!(camera_angle(0x2200, 225), (360, 316));
        assert_eq!(camera_angle(0x1C00, 225), (446, 391));
    }

    #[test]
    fn camera_angle_clamps_the_half_angle() {
        // `cmp bx,8*64 / jae` raises a zero/near-zero angle to 512, so the
        // index is 8 and the table entry 5210: `160*5210>>8 = 3256`,
        // `3256*225>>8 = 2861` (`AVID.ASM:68-76`). U2A's first drawn frame
        // passes fov = 0, so this end is shipped-reachable.
        assert_eq!(camera_angle(0, 225), (3256, 2861));
        // `cmp bx,16384 / jb / mov bx,16383` caps the half-angle at 16383,
        // index 255, entry 1, so both multipliers truncate to 0. Without the
        // cap the index would run off the 256-word table.
        assert_eq!(camera_angle(32768, 225), (0, 0));
        assert_eq!(camera_angle(65535, 225), (0, 0));
    }

    #[test]
    fn mulmatrices_keeps_the_low_32_bits_then_sar_14() {
        // `mulmacro` keeps `eax`, the low 32 bits of each `imul`
        // (`ACALC.ASM:52-63`). With apply = diag(1<<20, UNIT, UNIT) and dest
        // = identity, the [0][0] term is 2^20 * 2^14 = 2^34: its low 32 bits
        // are zero, so `sar 14` yields 0, where a 64-bit accumulation would
        // yield 2^20 = 1_048_576.
        let apply = Matrix {
            m: [1 << 20, 0, 0, 0, UNIT, 0, 0, 0, UNIT],
            ..Default::default()
        };
        let mut dest = Matrix::identity();
        apply_matrix(&mut dest, &apply);
        assert_eq!(dest.m[0], 0);
        assert_eq!(dest.m[4], UNIT);
        assert_eq!(dest.m[8], UNIT);

        // The asm reads the matrix with `imul dword ptr`, not a signed word
        // (`ACALC.ASM:52-63`). A 0x8000 entry is +32768: 32768*UNIT = 2^29,
        // `sar 14` = 32768. Read as a signed word it would be -32768*UNIT =
        // -2^29, and the result -32768.
        let apply = Matrix {
            m: [0x8000, 0, 0, 0, UNIT, 0, 0, 0, UNIT],
            ..Default::default()
        };
        let mut dest = Matrix::identity();
        apply_matrix(&mut dest, &apply);
        assert_eq!(dest.m[0], 32768);
    }

    #[test]
    fn apply_matrix_multiplies_apply_times_dest() {
        // In UNIT units a = [[1,1,0],[0,1,0],[0,0,1]] and
        // d = [[1,0,0],[1,1,0],[0,0,1]]. `apply * dest` is
        // [[2,1,0],[1,1,0],[0,0,1]]; the reversed order would give
        // [[1,1,0],[1,2,0],[0,0,1]] (`ACALC.ASM:391-393`).
        let apply = Matrix {
            m: [UNIT, UNIT, 0, 0, UNIT, 0, 0, 0, UNIT],
            ..Default::default()
        };
        let mut dest = Matrix {
            m: [UNIT, 0, 0, UNIT, UNIT, 0, 0, 0, UNIT],
            ..Default::default()
        };
        apply_matrix(&mut dest, &apply);
        assert_eq!(dest.m, [2 * UNIT, UNIT, 0, UNIT, UNIT, 0, 0, 0, UNIT]);
    }

    #[test]
    fn apply_matrix_rotates_the_position_with_the_full_64_bit_product() {
        // `rotatesingle` reads the matrix entries as dwords (`imul dword
        // ptr`) and keeps the full 64-bit product through
        // `shrd ebx,ecx,14` (`ACALC.ASM:415-468`). m[0] = 1<<20 times
        // x = 1<<20 is 2^40, so the rotated x is 2^26; a low-32 multiply
        // would leave 0. The translation is added afterwards.
        let apply = Matrix { m: [1 << 20, 0, 0, 0, UNIT, 0, 0, 0, UNIT], x: 7, y: 11, z: 13 };
        let mut dest = Matrix { x: 1 << 20, y: 5, z: 9, ..Matrix::identity() };
        apply_matrix(&mut dest, &apply);
        assert_eq!(dest.x, (1 << 26) + 7);
        assert_eq!(dest.y, 5 + 11);
        assert_eq!(dest.z, 9 + 13);
    }

    #[test]
    fn rotate_vertices_sign_extends_matrix_entries() {
        // `movsx eax, word ptr ds:[si+rmatrix_m+2*0]` (`ACALC.ASM:521`)
        // reads 0x8000 as -32768, so x = 1 gives -32768 >> 14 = -2. Reading
        // the dword 32768 instead would give +2. The normal index is copied.
        let m = Matrix { m: [0x8000, 0, 0, 0, UNIT, 0, 0, 0, UNIT], x: 10, y: 20, z: 30 };
        let verts = [Vertex { x: 1, y: 2, z: 3, normal: 7 }];
        let mut out = [Rotated::default()];
        rotate_vertices(&mut out, &verts, &m);
        assert_eq!(out[0], Rotated { x: 8, y: 22, z: 33, normal: 7 });
    }

    #[test]
    fn rotate_vertices_keeps_the_full_64_bit_product() {
        // UNIT * (1<<20) = 2^34; `shrd ebx,ecx,14` returns 2^20 where a
        // 32-bit product would overflow (`ACALC.ASM:533`).
        let mut out = [Rotated::default()];
        rotate_vertices(&mut out, &[vertex(1 << 20, 0, 0)], &Matrix::identity());
        assert_eq!(out[0].x, 1 << 20);
    }

    #[test]
    fn rotate_normals_sign_extends_matrix_entries_and_ignores_translation() {
        // The normals path multiplies the matrix entry as a signed word
        // (`imul word ptr`, `ACALC.ASM:631`): 0x8000 * 1 >> 14 = -2. No
        // position is added, and the result is a 16-bit store (`:643`).
        let m = Matrix { m: [0x8000, 0, 0, 0, UNIT, 0, 0, 0, UNIT], x: 999, y: 999, z: 999 };
        let normals = [Normal { x: 1, y: 2, z: 3 }];
        let mut out = [RotatedNormal::default()];
        rotate_normals(&mut out, &normals, &m);
        assert_eq!(out[0], RotatedNormal { x: -2, y: 2, z: 3 });
    }

    #[test]
    fn project_divides_with_truncation() {
        // `idiv` truncates toward zero (`ACALC.ASM:784`): (-7 * 145) / 3 is
        // -338, not the -339 a floor division gives. z = 3 is inside the
        // clip range so it is not substituted.
        let p = Projection {
            mul_x: 145,
            mul_y: 1,
            add_x: 0,
            add_y: 0,
            clip_x: [-10_000, 10_000],
            clip_y: [-10_000, 10_000],
            clip_z: [1, 1000],
        };
        let src = rotated(-7, 0, 3);
        let mut out = PVert::default();
        project(&src, &mut out, &p);
        assert_eq!(out.x, -338);
        assert_eq!(out.y, 0);
        assert_eq!(out.vf, 0);
    }

    #[test]
    fn project_uses_each_axis_add_and_keeps_far_z() {
        // Distinct, nonzero adds: y takes add_y and x takes add_x. With
        // x=10, y=20, z=500 and mul 500, x = 10+100 = 110 and y = 20+40 =
        // 60; swapping the adds would give x = 10+40 = 50 and y = 20+100 =
        // 120.
        let p = Projection {
            mul_x: 500,
            mul_y: 500,
            add_x: 100,
            add_y: 40,
            clip_x: [-1000, 1000],
            clip_y: [-1000, 1000],
            clip_z: [100, 1000],
        };
        let src = rotated(10, 20, 500);
        let mut out = PVert::default();
        project(&src, &mut out, &p);
        assert_eq!((out.x, out.y, out.vf), (110, 60, 0));

        // A far vertex is flagged but its z is *not* substituted
        // (`ACALC.ASM:779-781` only sets the flag): with x=3 and z=1001,
        // x = 3*1000/1001 = 2; clamping z to 1000 would give 3.
        let p = Projection {
            mul_x: 1000,
            mul_y: 1,
            add_x: 0,
            add_y: 0,
            clip_x: [-1000, 1000],
            clip_y: [-1000, 1000],
            clip_z: [100, 1000],
        };
        let src = rotated(3, 0, 1001);
        let mut out = PVert::default();
        project(&src, &mut out, &p);
        assert_eq!((out.x, out.y, out.vf), (2, 0, VF_FAR));
    }

    #[test]
    fn project_sets_the_visibility_flags_at_the_clip_boundaries() {
        // mul 500 with z = 500 makes the projection the identity for the
        // in-range vertices. The asm's comparisons are `jng`/`jnl`: equality
        // with a clip is *inside* (`ACALC.ASM:786-804`).
        let p = Projection {
            mul_x: 500,
            mul_y: 500,
            add_x: 0,
            add_y: 0,
            clip_x: [-10, 10],
            clip_y: [-20, 20],
            clip_z: [100, 1000],
        };
        let project_xyz = |x: i32, y: i32, z: i32| {
            let mut out = PVert::default();
            project(&rotated(x, y, z), &mut out, &p);
            out
        };
        assert_eq!(project_xyz(0, 20, 500), PVert { x: 0, y: 20, vf: 0 });
        assert_eq!(project_xyz(0, 21, 500).vf, VF_DOWN);
        assert_eq!(project_xyz(0, -20, 500), PVert { x: 0, y: -20, vf: 0 });
        assert_eq!(project_xyz(0, -21, 500).vf, VF_UP);
        assert_eq!(project_xyz(10, 0, 500), PVert { x: 10, y: 0, vf: 0 });
        assert_eq!(project_xyz(11, 0, 500).vf, VF_RIGHT);
        assert_eq!(project_xyz(-10, 0, 500), PVert { x: -10, y: 0, vf: 0 });
        assert_eq!(project_xyz(-11, 0, 500).vf, VF_LEFT);
        // z == min is inside; below it the near flag is set and z is
        // replaced by the clip before the divisions.
        assert_eq!(project_xyz(0, 0, 100).vf, 0);
        assert_eq!(project_xyz(0, 0, 99).vf, VF_NEAR);
        // z == max is inside; above it only the far flag is set.
        assert_eq!(project_xyz(0, 0, 1000).vf, 0);
        assert_eq!(project_xyz(0, 0, 1001).vf, VF_FAR);
        // The flags combine, and the near substitution feeds the divides:
        // x = 11*500/100 = 55 and y = 21*500/100 = 105 are both outside.
        let near = project_xyz(11, 21, 99);
        assert_eq!(near.vf, VF_NEAR | VF_DOWN | VF_RIGHT);
        assert_eq!((near.x, near.y), (55, 105));
    }

    #[test]
    fn project_stores_16_bits_after_flagging_the_full_value() {
        // `mov es:[di+pvlist_x],ax` stores the word, but the comparisons use
        // the full 32-bit `eax` (`ACALC.ASM:798-804`): 40_000 is inside
        // `[-100_000, 30_000]`? It is not — it is right of 30_000 — while
        // the stored word, -25_536, is inside.
        let p = Projection {
            mul_x: 500_000,
            mul_y: 0,
            add_x: 0,
            add_y: 0,
            clip_x: [-100_000, 30_000],
            clip_y: [0, 0],
            clip_z: [100, 1000],
        };
        let src = rotated(40, 0, 500);
        let mut out = PVert::default();
        project(&src, &mut out, &p);
        assert_eq!(out.x, -25_536);
        assert_eq!(out.vf, VF_RIGHT);
    }

    #[test]
    fn project_all_projects_and_ands_every_vertex_flag() {
        // mul 500 with z = 500 is the identity for these vertices; the clips
        // are the boundary test's.
        let p = Projection {
            mul_x: 500,
            mul_y: 500,
            add_x: 0,
            add_y: 0,
            clip_x: [-10, 10],
            clip_y: [-20, 20],
            clip_z: [100, 1000],
        };

        // One vertex inside and one right-and-down: the AND is 0.
        let mut out = [PVert::default(); 2];
        assert_eq!(project_all(&[rotated(0, 0, 500), rotated(11, 21, 500)], &mut out, &p), 0);
        assert_eq!(out[0], PVert { x: 0, y: 0, vf: 0 });
        assert_eq!(out[1], PVert { x: 11, y: 21, vf: VF_DOWN | VF_RIGHT });

        // Two vertices that both share VF_DOWN: the AND keeps it.
        let both = [rotated(0, 21, 500), rotated(11, 21, 500)];
        assert_eq!(project_all(&both, &mut out, &p), VF_DOWN);

        // The asm starts `ax` at 0xFFFF and skips the loop for an empty list.
        assert_eq!(project_all(&[], &mut [], &p), 0xFFFF);
    }

    #[test]
    fn single_z_uses_full_32_bit_matrix_entries() {
        // `_calc_singlez` reads m[6] with `mov eax`, not `movsx`
        // (`ACALC.ASM:593`): 1<<20 stays 1<<20, and the full product 2^40
        // becomes 2^26. The remaining rows are the local vertex's y and z;
        // m[6]*x + m[7]*y + m[8]*z = 2^40 + 9*UNIT, then the position z.
        let m = Matrix { m: [UNIT, 0, 0, 0, UNIT, 0, 1 << 20, 0, UNIT], x: 0, y: 0, z: 100 };
        assert_eq!(single_z(&[vertex(1 << 20, 5, 9)], 0, &m), (1 << 26) + 9 + 100);
    }

    #[test]
    fn single_z_selects_the_indexed_vertex() {
        let m = Matrix { z: 50, ..Matrix::identity() };
        // The matrix's z row is [0,0,UNIT], so it transforms the z coordinate.
        let verts = [vertex(0, 0, 7), vertex(0, 0, 9)];
        assert_eq!(single_z(&verts, 0, &m), 7 + 50);
        assert_eq!(single_z(&verts, 1, &m), 9 + 50);
    }
}
