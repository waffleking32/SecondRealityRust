//! HARD's c-prefixed vector engine: the fork of GLENZ's library that `B.EXE`
//! runs (`HARD/VEC.ASM`, which includes `VNEW.ASM`, `VMATH.ASM` and
//! `VDATA.ASM`).
//!
//! The engine is a fork, not a wrapper (`docs/fidelity/hard.md`):
//!
//! - [`math::rotate`] uses the source's own axis stores: x is the third sum
//!   (matrix words 12/14/16), y the second (6/8/10), z the first (0/2/4)
//!   (`VEC.ASM:172-176`), where GLENZ's rotation assigns x to the first sum,
//!   y to the third and z to the second (`GLENZ/VEC.ASM:155-160`).
//! - [`math::project`] runs the 240/200/160/64/512 profile `VDATA.ASM:30-36`
//!   defines and keeps the raw x/y words `projlist` stores at `+12`/`+14`
//!   (`VEC.ASM:214,226`) for the clipper.
//! - [`math::rotate_project`] is `crotprojlist` (`VEC.ASM:250-330`): the same
//!   rotation followed by a **16-bit** `imul word ptr`/`idiv bx` projection
//!   that reads only the low words, writes no raw fields and sets no flags.
//! - [`poly::shoelace`] is `checkhiddenbx`'s full signed sum over every edge
//!   (`VEC.ASM:369-400`), not GLENZ's first-three cross product.
//! - [`poly::clip_easy_list`] is `cclipeasypolylist`'s `clipz = 4000` 2-D
//!   clipper (`VEC.ASM:833-902`), which has no GLENZ counterpart.
//! - [`span::Compositor`] paints 128 rows opaquely into the A2D0 band
//!   (`VNEW.ASM`), where GLENZ's compositor is 200 rows transparent over
//!   `bgpic`.
//!
//! The tables and the matrix builder are shared with GLENZ because the files
//! are identical: `glenz::tables::{SIN, COS}` (`VMATHSIN.ASM` is
//! `MATHSIN.INC`; `SIN1024.INC` is the shared [`crate::parts::sin1024`]) and
//! `glenz::math::matrix_yxz` (`VMATH.ASM` differs from `MATH.ASM` only in its
//! include/segment wrapper). Nothing else is shared: HARD's colour path is
//! index-only — `demo_norm` is a no-op and `demo_glz` is dead (`VEC.ASM:486-580`),
//! so there is no slot allocator and no DAC shading.

pub mod dots;
pub mod math;
pub mod part;
pub mod poly;
pub mod span;
