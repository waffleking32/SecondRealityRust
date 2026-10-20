//! GLENZ (`GLENZ/MAIN.C` over the part's own `VEC.ASM`, `MATH.ASM`,
//! `MATHSIN.INC` and `NEW.ASM`).
//!
//! GLENZ carries its own vector and span library rather than using Phase 6's
//! `VISU`: the two are different engines, with different fixed-point units,
//! different projection variables and a different compositor. The module tree
//! grows a file per layer of that library:
//!
//! - `math` — `cmatrix_yxz`/`calcmatrix`, `crotlist`, `cprojlist` and
//!   `ccliplist` (`MATH.ASM`, `VEC.ASM`), with the asm's 16/32/64-bit widths.
//! - `tables` — the source's `sintable16`/`costable16` (`MATHSIN.INC`) as
//!   [`tables::SIN`] and [`tables::COS`].
//!
//! `poly` (the point-list builder and colour allocator), `span` (the
//! transparent compositor) and `intro` (the zoomer and the FC bounce) are
//! later tasks' files; the part driver lands with them.

pub mod math;
pub mod tables;
