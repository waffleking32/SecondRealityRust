//! The VISU vector engine (`VISU/ACALC.ASM`, `VISU/ADRAW*.ASM`).
//!
//! `math` is the fixed-point core: the matrix multiply and apply, the vertex
//! and normal rotations, the projection window and per-vertex clipping flags,
//! and the camera-angle table. The polygon pipeline (`draw`) and the scene
//! driver are the rest of the engine.
//!
//! Nothing outside this module's own tests names its items yet: the pipeline
//! and driver are the callers, and they land later in the phase.

// Every `math` item is unused until the polygon pipeline (Task 4) and the
// scene driver (Task 5) consume it; this allow is scoped to `math` alone and
// is removed once Task 5's driver calls the maths (plain
// `cargo clippy -D warnings` then passes without it).
#[allow(dead_code)]
pub mod math;
