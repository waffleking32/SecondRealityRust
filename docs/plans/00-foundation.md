# Phase 0 — Foundation & Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the software-VGA engine skeleton, a synthetic test card that exercises every visually-significant hardware behaviour, and the reference-capture/comparison harness — so that every later phase has somewhere to put a part and a way to prove it correct.

**Architecture:** `sr-engine` is a dependency-free library modelling the original's visible output as an indexed surface plus a per-scanline palette plus a view transform. It owns `Surface`, `View`, `Palette`, `Tick`, the copper callbacks, the `Part` trait, and beam compose. `bins/secondreality` owns the SDL2 window, keeping the engine free of display dependencies. `xtask` is a separate CLI holding the reference oracle behind a trait.

**Tech Stack:** Rust 1.99 (edition 2021), SDL2 2.30 via the `sdl2` crate, `serde`/`serde_json`, `ureq`, `png`, `clap`.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md`

## Global Constraints

- Rust **edition 2021**; `rust-version = "1.85"`. Every crate manifest sets `license = "MIT"` (spec §14).
- `sr-engine` has **zero runtime dependencies**. SDL2 must not appear in it (D3, §4.2). `[dev-dependencies]` are exempt from this: they never enter a consumer's dependency graph.
- **`rstest = "0.27"` is the test dependency of choice** — used for `#[case]` tables and `#[fixture]`s where they read better than a hand-rolled loop or helper. It is a dev-dependency only. Not every test becomes an rstest; a test that reads clearly as plain `#[test]` stays plain.
- The reference of record is the 1993 retail release in `reference/original/` — never a build from the source tree (D12). Those files are read-only.
- Palette is **6-bit per channel internally**, widened only for upload (CLAUDE.md, §4.2).
- The logic tick is **exactly 70.086 Hz** and **never spin-waits** (§5.3, §6.1).
- Surface is allocated at the largest geometry in use, **640×400** (§4.2).
- Displayed geometries: 320×200, 320×400, 640×400, all indexed 8-bit (§2.3).
- Pixel aspect defaults to **4:3**; square pixels are an opt-in toggle (§4.2, D4).
- Assets are read in their original formats; no conversion (§9). *(Not exercised until Phase 1.)*
- The three copper slots are **not** a no-op; their ordering is fixed (§4.2, D6).
- An approximation is only permitted with an entry in `docs/fidelity/APPROXIMATIONS.md` recording the measured difference (CLAUDE.md).
- When porting anything from the original, cite the upstream file and line in the commit message (CLAUDE.md).
- Every commit ends with: `Co-Authored-By: Claude Code <noreply@anthropic.com>`
- Oracle-dependent tasks are marked **[oracle]**. Everything else must complete with no reference oracle available (D10).

## Review Focus

Five input classes the spec implies but whose behaviour no exit criterion pins down. Each has a test in the task that owns the code.

1. **A long wall-clock stall** (laptop suspend, debugger pause, a slow first frame) must not make the engine emit a burst of catch-up ticks. Expected: excess time is dropped. → Task 4, `a_stall_is_clamped_and_does_not_produce_a_burst`.
2. **A part writing outside the surface** must be ignored — not panic, and not wrap into the neighbouring scanline (the flat-index bug). → Task 1, `set_beyond_width_is_ignored_and_does_not_wrap`.
3. **A view whose origin runs past the end of the surface** must wrap, because the original's circular-buffer tricks depend on the CRTC wrapping. → Task 3, `source_index_wraps_past_the_end`.
4. **A split scanline outside the display** must be inert, not a panic and not a silently wrong row. → Task 3, `split_beyond_height_is_inert`.
5. **A frame height not divisible by the band count** must still partition the frame exactly once — no gap, no overlap. → Task 5, `band_range_partitions_exactly_for_any_band_count`.

## Decisions this plan makes that the spec leaves open

These refine the spec rather than changing it. Each is recorded in `docs/decisions.md` when the relevant task lands.

1. **Presentation lives in `bins/secondreality`, not `sr-engine`.** The spec's §4.2 puts "Presentation" under `sr-engine`; putting the SDL2 dependency there would make the engine untestable without SDL. No `sr-present` crate is introduced — the engine keeps the pure raster maths, the binary owns the window.
2. **`View` gains a `width` field.** §4.2's struct sketch lists height but not width; 320 and 640 wide displays both exist (§2.3).
3. **`View::split` is `Option<Box<Split>>`, and neither `View` nor `Split` is `Copy`.** A `Split` containing a `View` containing a `Split` is an infinitely-sized type, hence the `Box`. That `Box` is also why they are `Clone` only: `Box<T>` is never `Copy` — it owns its contents, and copying it would permit a double free — so `View` cannot be `Copy`, and `Split`, which contains a `View`, cannot be either. Nothing needs a copy: `effective` returns `&View`, `surface_row` and `source_index` take `&self`, and `Engine::new`/`Runner::new` take the view by value, which is a move.
4. **`Part` gains `name()`;** `init` returns `Result<(), PartError>` rather than `Result<()>` so the runner can name the failing part.
5. **Music is not in `PartCtx` in Phase 0.** It arrives in Phase 3 (spec §7). Adding fields to `PartCtx` is additive and breaks no part.
6. **The overscan border is a fixed pad in the presenter**, not a modelled overscan region. First entry in `APPROXIMATIONS.md`.
7. **`xtask` is invoked as `cargo xtask`** via a `.cargo/config.toml` alias, not as a bare `xtask` binary.
8. **The composed frame is the displayed area only** — no full-frame overscan synthesis. `IndexedFrame` is exactly `view.width × view.height`.
9. **The reference of record is the 1993 retail release**, vendored at `reference/original/`, not a build from the source tree. Fidelity means "looks like the demo people ran". Recorded as **D12** in `docs/decisions.md`, which also records that `REALITY.FC` is byte-identical to the source tree's copy.
10. **`rstest` is a dev-dependency.** `sr-engine`'s zero-*runtime*-dependency constraint is what keeps it usable without SDL2; a test-only crate does not violate it. Used for `#[case]` tables and `#[fixture]`s where they read better than a loop or helper.

## Out of scope for Phase 0

Loaders for any original asset format; the music player; any real demo part; the vector engine; the shell's sequencer and settings screen; the vault. Those are Phases 1–9.

Two spec sections name work that deliberately does **not** land here, so a later session does not read their absence as an oversight:

- **§8.3, the event stream.** Palette writes, view changes and the scanline each takes effect on are the technique that turns "the picture is slightly off" into a specific number. It is deferred to **Phase 2**, where the first part is diffed against the original and there is something real to compare an event log against. Adding it in Phase 0 would mean designing the event vocabulary against a synthetic test card, which is the one thing that cannot validate it.
- **§6.2, `dis_waitb`'s return value and counter-reset semantics.** This lands in **Phase 2**, with the first part that uses it. No Phase 0 part waits on anything, and the semantics (the counter reset discards an overrun, so a part that misses its budget sees "1 frame", not 2) are subtle enough that pinning them without a caller to check against risks encoding the wrong model.

---

## File structure

```
Cargo.toml                          workspace root
.cargo/config.toml                  `cargo xtask` alias
crates/sr-engine/
  Cargo.toml
  src/lib.rs                        module list + re-exports
  src/surface.rs                    Surface, MAX_WIDTH/MAX_HEIGHT
  src/palette.rs                    Palette, PaletteTimeline, dac_expand
  src/view.rs                       View, Split
  src/tick.rs                       Tick
  src/compose.rs                    IndexedFrame, band_range, compose_band
  src/copper.rs                     CopperFn, CopperRegistry
  src/part.rs                       Part, PartStatus, PartError, PartCtx
  src/engine.rs                     Engine, Runner
  src/present.rs                    Aspect, PresentOptions, FrameBuffer, present
  src/testcard.rs                   TestCard
bins/secondreality/
  Cargo.toml
  src/main.rs                       entry point
  src/cli.rs                        argument parsing
  src/headless.rs                   frame dumping
  src/window.rs                     SDL2 window + pacing
  tests/headless.rs                 end-to-end headless smoke test
xtask/
  Cargo.toml
  src/main.rs                       clap CLI
  src/oracle/mod.rs                 Oracle trait, CapturedFrame
  src/oracle/dosbox.rs              dosbox-automation REST client
  src/reference.rs                  `reference capture`
  src/compare.rs                    `compare`
docs/fidelity/part-sequence.md      verified part table (Task 10)
docs/fidelity/APPROXIMATIONS.md     approximations register
reference/original/                 the 1993 retail release — the oracle's input (D12)
reference/<name>/                   captured keyframes + manifests
out/                                headless render output (gitignored)
```

---

### Task 1: Workspace skeleton and `Surface`

**Files:**
- Create: `Cargo.toml`, `.cargo/config.toml`, `crates/sr-engine/Cargo.toml`, `crates/sr-engine/src/lib.rs`, `crates/sr-engine/src/surface.rs`
- Modify: `.gitignore`

**Interfaces:**
- Produces: `sr_engine::surface::{Surface, MAX_WIDTH, MAX_HEIGHT}`, `sr_engine::MAX_WIDTH`, `sr_engine::MAX_HEIGHT`

```rust
pub const MAX_WIDTH: usize = 640;
pub const MAX_HEIGHT: usize = 400;

pub struct Surface { /* width, height, pixels: Vec<u8> */ }

impl Surface {
    pub fn new(width: usize, height: usize) -> Surface;
    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
    pub fn pixels(&self) -> &[u8];
    pub fn pixels_mut(&mut self) -> &mut [u8];
    /// `None` when out of bounds.
    pub fn get(&self, x: usize, y: usize) -> Option<u8>;
    /// Out-of-bounds writes are ignored. Never panics, never wraps to another row.
    pub fn set(&mut self, x: usize, y: usize, v: u8);
    pub fn clear(&mut self, v: u8);
    pub fn row(&self, y: usize) -> &[u8];
    pub fn row_mut(&mut self, y: usize) -> &mut [u8];
}
```

- [ ] **Step 1: Create the workspace and crate manifests**

Root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = ["crates/sr-engine"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
rust-version = "1.85"

[workspace.dependencies]
sr-engine = { path = "crates/sr-engine" }
```

`crates/sr-engine/Cargo.toml`:

```toml
[package]
name = "sr-engine"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]

[dev-dependencies]
rstest = "0.27"
```

The empty `[dependencies]` section is deliberate — it is the constraint made visible. `rstest` sits in `[dev-dependencies]`, so it is compiled for `cargo test` and never for a consumer of `sr-engine`.

`.cargo/config.toml`:

```toml
[alias]
xtask = "run --package xtask --"
```

Append `/out/` to `.gitignore`.

- [ ] **Step 2: Write the failing tests**

In `crates/sr-engine/src/surface.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_round_trip() {
        let mut s = Surface::new(4, 2);
        s.set(2, 1, 42);
        assert_eq!(s.get(2, 1), Some(42));
        assert_eq!(s.get(0, 0), Some(0));
    }

    #[test]
    fn set_beyond_width_is_ignored_and_does_not_wrap() {
        let mut s = Surface::new(4, 2);
        s.set(4, 0, 42);
        s.set(999, 0, 43);
        assert_eq!(s.row(1), &[0, 0, 0, 0]);
        assert_eq!(s.get(4, 0), None);
    }

    #[test]
    fn set_beyond_height_is_ignored() {
        let mut s = Surface::new(4, 2);
        s.set(0, 2, 42);
        s.set(0, 999, 43);
        assert_eq!(s.row(1), &[0, 0, 0, 0]);
    }

    #[test]
    fn clear_fills_every_pixel() {
        let mut s = Surface::new(3, 2);
        s.clear(7);
        assert_eq!(s.pixels(), &[7; 6]);
    }

    #[test]
    fn row_mut_does_not_touch_neighbours() {
        let mut s = Surface::new(3, 2);
        s.row_mut(0).fill(9);
        assert_eq!(s.row(0), &[9, 9, 9]);
        assert_eq!(s.row(1), &[0, 0, 0]);
    }

    #[test]
    fn new_is_zeroed() {
        assert_eq!(Surface::new(MAX_WIDTH, MAX_HEIGHT).pixels().len(), 256_000);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p sr-engine surface`
Expected: compile error — `cargo` cannot find `crates/sr-engine/target`, `Surface` is not defined.

- [ ] **Step 4: Implement `Surface`, `MAX_WIDTH`, `MAX_HEIGHT`**

`set` computes the flat index only after checking `x < self.width && y < self.height`; that check is what stops the wrap in Review Focus #2. `row`/`row_mut` slice the flat buffer at `y * width`. `lib.rs` declares `pub mod surface;` and re-exports `pub use surface::{Surface, MAX_HEIGHT, MAX_WIDTH};`.

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: `6 passed`.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock Cargo.toml .cargo/config.toml .gitignore crates/sr-engine
git commit -m "$(cat <<'EOF'
Add workspace skeleton and the indexed Surface

sr-engine is deliberately dependency-free; the empty [dependencies]
section makes that constraint visible in the manifest.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `Palette` and the DAC widening rule

**Files:**
- Create: `crates/sr-engine/src/palette.rs`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Produces: `sr_engine::palette::{Palette, dac_expand}`

```rust
/// Widen a 6-bit DAC value to 8 bits by replicating the top bits.
/// Input above 63 is masked; the DAC had no eighth bit to receive it.
pub const fn dac_expand(v: u8) -> u8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette { /* entries: [[u8; 3]; 256], each component 0..=63 */ }

impl Palette {
    pub const BLACK: Palette;
    pub fn new() -> Palette;
    /// 768 bytes of 8-bit RGB, as stored by `.PAL`, LBM and the S3M modules.
    /// Each component is shifted down by 2 — the DAC's 6 bits are the top 6.
    pub fn from_vga(bytes: &[u8]) -> Palette;
    /// Round-trips `from_vga` for input already in 0..=63.
    pub fn to_vga(&self) -> [u8; 768];
    pub fn entry(&self, index: u8) -> [u8; 3];
    /// Components above 63 are masked with `& 0x3F`.
    pub fn set_entry(&mut self, index: u8, rgb: [u8; 3]);
    pub fn copy_from(&mut self, other: &Palette);
    /// 256 RGBA quads, 1024 bytes, through `dac_expand`.
    pub fn write_rgba(&self, out: &mut [u8; 1024]);
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/palette.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn from_vga_keeps_six_bit_precision() {
        let mut vga = [0u8; 768];
        vga[0..3].copy_from_slice(&[255, 128, 4]);
        let p = Palette::from_vga(&vga);
        assert_eq!(p.entry(0), [63, 32, 1]);
    }

    #[test]
    fn to_vga_round_trips_values_already_in_range() {
        let mut p = Palette::new();
        p.set_entry(5, [63, 32, 1]);
        let vga = p.to_vga();
        assert_eq!(&vga[15..18], &[252, 128, 4]);
        assert_eq!(Palette::from_vga(&vga).entry(5), [63, 32, 1]);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(1, 4)]
    #[case(16, 65)]
    #[case(32, 130)]
    #[case(63, 255)]
    // Above six bits the DAC had nowhere to put the extra bits, so they are masked.
    #[case(64, 0)]
    #[case(255, 255)]
    fn dac_expand_replicates_the_top_bits(#[case] input: u8, #[case] expected: u8) {
        assert_eq!(dac_expand(input), expected);
    }

    #[test]
    fn set_entry_masks_instead_of_overflowing_into_the_next_channel() {
        let mut p = Palette::new();
        p.set_entry(1, [255, 64, 63]);
        assert_eq!(p.entry(1), [63, 0, 63]);
    }

    #[test]
    fn write_rgba_emits_256_quads() {
        let mut p = Palette::new();
        p.set_entry(0, [63, 0, 32]);
        p.set_entry(255, [0, 63, 0]);
        let mut out = [0u8; 1024];
        p.write_rgba(&mut out);
        // dac_expand(32) = (32 << 2) | (32 >> 4) = 128 | 2 = 130.
        assert_eq!(&out[0..4], &[255, 0, 130, 255]);
        assert_eq!(&out[1020..1024], &[0, 255, 0, 255]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine palette`
Expected: compile error — `Palette` and `dac_expand` are not defined.

- [ ] **Step 3: Implement `Palette` and `dac_expand`**

`dac_expand` is `((v & 0x3F) << 2) | ((v & 0x3F) >> 4)` — masking first keeps it a total function on `u8`. `from_vga` reads three bytes per entry and shifts each right by 2. `write_rgba` writes `[dac_expand(r), dac_expand(g), dac_expand(b), 255]`. `BLACK` is `[[0, 0, 0]; 256]` as a `const`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/palette.rs
git commit -m "$(cat <<'EOF'
Add the 6-bit Palette and the DAC widening rule

Six bits are kept internally and widened only for output, matching the
VGA DAC. The widening rule is (v << 2) | (v >> 4); Task 12 checks it
against frames from the emulated original.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `View` — the CRTC replacement

**Files:**
- Create: `crates/sr-engine/src/view.rs`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Produces: `sr_engine::view::{View, Split}`

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub width: u16,           // displayed width in pixels
    pub height: u16,          // displayed height in scanlines
    pub start_addr: u16,      // CRTC display start (0x0C/0x0D), in pixels
    pub x_offset: u16,        // planar byte-to-pixel offset; added to start_addr
    pub y_line_offset: u16,   // pixel stride between displayed scanlines (CRTC 0x13)
    pub pan: u8,              // fine horizontal shift, 0..=7 (AC 0x13)
    pub double_scan: bool,    // CRTC reg 9 bit 7
    pub split: Option<Box<Split>>,
    pub border: Option<u8>,   // overscan colour (AC 0x11)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split {
    pub scanline: u16,   // first displayed scanline using `view`
    pub view: View,
}

impl View {
    /// Origin at 0, `y_line_offset = width`, no split, no border.
    pub fn new(width: u16, height: u16) -> View;
    /// Sets `x_offset` and zeroes `start_addr`.
    pub fn set_origin(&mut self, pixel: u16);
    /// The view in force at displayed scanline `y`, walking the split chain.
    /// A split at or beyond `height` is inert. `Box` makes cycles impossible.
    pub fn effective(&self, y: u16) -> &View;
    /// Surface row sampled by displayed scanline `y`, after double-scan.
    pub fn surface_row(&self, y: u16) -> u16;
    /// Index into a surface of `surface_len` pixels for displayed pixel (x, y).
    /// Wraps, like the CRTC address counter.
    pub fn source_index(&self, surface_len: usize, x: u16, y: u16) -> usize;
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/view.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[fixture]
    fn v320() -> View { View::new(320, 200) }

    #[rstest]
    fn new_defaults_to_a_contiguous_origin(v320: View) {
        assert_eq!(v320.start_addr, 0);
        assert_eq!(v320.x_offset, 0);
        assert_eq!(v320.y_line_offset, 320);
        assert_eq!(v320.pan, 0);
        assert!(!v320.double_scan);
        assert!(v320.split.is_none());
        assert!(v320.border.is_none());
    }

    #[rstest]
    fn source_index_follows_the_line_offset(mut v320: View) {
        v320.set_origin(10);
        assert_eq!(v320.source_index(64_000, 3, 2), 10 + 2 * 320 + 3);
    }

    #[rstest]
    fn source_index_wraps_past_the_end() {
        let mut v = View::new(4, 1);
        v.start_addr = 6;
        assert_eq!(v.source_index(8, 3, 0), 1);
    }

    #[rstest]
    fn pan_shifts_by_one_pixel_per_unit(mut v320: View) {
        v320.pan = 5;
        assert_eq!(v320.source_index(64_000, 0, 0), 5);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(1, 0)]
    #[case(2, 1)]
    #[case(399, 199)]
    fn double_scan_maps_two_scanlines_to_one_surface_row(
        #[case] screen_y: u16,
        #[case] source_row: u16,
    ) {
        let mut v = View::new(320, 200);
        v.double_scan = true;
        assert_eq!(v.surface_row(screen_y), source_row);
    }

    #[rstest]
    fn single_scan_advances_one_row_per_scanline() {
        let v = View::new(320, 200);
        assert_eq!(v.surface_row(0), 0);
        assert_eq!(v.surface_row(1), 1);
        assert_eq!(v.surface_row(199), 199);
    }

    #[rstest]
    fn split_switches_view_at_its_scanline(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(320 * 100);
        v320.split = Some(Box::new(Split { scanline: 120, view: lower }));
        assert_eq!(v320.effective(119).x_offset, 0);
        assert_eq!(v320.effective(120).x_offset, 320 * 100);
    }

    #[rstest]
    fn split_beyond_height_is_inert(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(999);
        v320.split = Some(Box::new(Split { scanline: 500, view: lower }));
        assert_eq!(v320.effective(199).x_offset, 0);
        assert_eq!(v320.source_index(64_000, 0, 199), 199 * 320);
    }

    #[rstest]
    fn nested_splits_resolve_to_the_deepest_match() {
        let mut mid = View::new(320, 200);
        mid.set_origin(1000);
        let mut low = View::new(320, 200);
        low.set_origin(2000);
        mid.split = Some(Box::new(Split { scanline: 150, view: low }));
        let mut top = View::new(320, 200);
        top.split = Some(Box::new(Split { scanline: 50, view: mid }));
        assert_eq!(top.effective(49).x_offset, 0);
        assert_eq!(top.effective(50).x_offset, 1000);
        assert_eq!(top.effective(150).x_offset, 2000);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine view`
Expected: compile error — `View` is not defined.

- [ ] **Step 3: Implement `View` and `Split`**

`effective` walks `while let Some(s) = &v.split { if y < s.scanline { break } v = &s.view }`. `surface_row` is `y >> v.double_scan as u16` on the effective view. `source_index` is `(start_addr + x_offset + surface_row(y) * y_line_offset + pan + x) % surface_len`.

**Note on `start_addr` + `x_offset`:** in the original these are the CRTC display-start register and the planar byte offset respectively; the port keeps both so event logs can cite the raw register value. `set_origin` is the convenience path and zeroes `start_addr`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/view.rs
git commit -m "$(cat <<'EOF'
Add View, the CRTC replacement

Models display start, line offset, fine pan, double-scan, line-compare
split and the overscan colour in one struct. Source addresses wrap, as
the CRTC address counter does — the original's circular-buffer effects
depend on it.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: `Tick` — the fixed 70.086 Hz clock

**Files:**
- Create: `crates/sr-engine/src/tick.rs`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Produces: `sr_engine::tick::Tick`

```rust
pub struct Tick { /* acc: u64, total: u64 */ }

impl Tick {
    /// VGA vertical refresh, in ticks per second × 1000.
    pub const HZ_MILLI: u64 = 70_086;
    /// Elapsed wall-clock beyond this is dropped, not caught up.
    pub const MAX_ELAPSED: Duration = Duration::from_millis(250);

    pub fn new() -> Tick;
    /// Feed elapsed wall-clock time; returns how many logic ticks are now due.
    /// Elapsed beyond `MAX_ELAPSED` is discarded.
    pub fn advance(&mut self, elapsed: Duration) -> u32;
    /// Ticks emitted since construction.
    pub fn total(&self) -> u64;
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/tick.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    /// One tick is 1e12 accumulator units at 70_086 units per nanosecond, so a
    /// period is 14,268,225.27 ns and the first whole tick lands at 14,268,225.
    #[rstest]
    #[case(0, 0)]
    #[case(14_268_224, 0)]
    #[case(14_268_225, 1)]
    #[case(1_000_000_000, 70)]
    fn advance_emits_whole_ticks_only(#[case] elapsed_ns: u64, #[case] ticks: u32) {
        let mut t = Tick::new();
        assert_eq!(t.advance(Duration::from_nanos(elapsed_ns)), ticks);
    }

    #[rstest]
    fn a_partial_tick_carries_into_the_next_call() {
        let mut t = Tick::new();
        assert_eq!(t.advance(Duration::from_nanos(14_268_224)), 0);
        assert_eq!(t.advance(Duration::from_nanos(1)), 1);
    }

    #[rstest]
    fn zero_elapsed_leaves_the_accumulator_intact() {
        let mut t = Tick::new();
        assert_eq!(t.advance(Duration::ZERO), 0);
        assert_eq!(t.advance(Duration::from_secs(1)), 70);
        assert_eq!(t.total(), 70);
    }

    #[rstest]
    fn a_thousand_seconds_yields_exactly_70086_ticks() {
        let mut t = Tick::new();
        let mut ticks = 0u64;
        for _ in 0..1000 { ticks += t.advance(Duration::from_secs(1)) as u64; }
        assert_eq!(ticks, 70_086);
        assert_eq!(t.total(), 70_086);
    }

    #[rstest]
    fn a_stall_is_clamped_and_does_not_produce_a_burst() {
        let mut t = Tick::new();
        // Simulates a laptop suspend or a debugger pause.
        let burst = t.advance(Duration::from_secs(600));
        assert!(burst <= 18, "a 600 s stall emitted {burst} ticks");
        assert!(burst >= 17, "a 250 ms window should still emit ~17 ticks");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine tick`
Expected: compile error — `Tick` is not defined.

- [ ] **Step 3: Implement `Tick`**

Accumulator arithmetic, exact and integer; no floats and no drift:

```rust
/// One logic tick is this many accumulator units.
const TICK_SCALE: u64 = 1_000_000_000_000;
/// An accumulator unit is `HZ_MILLI`-th of a unit per nanosecond:
/// ticks = elapsed_ns * 70_086 / 1e12.
const NS_TO_ACC: u64 = 70_086;
```

`advance` clamps `elapsed` to `MAX_ELAPSED` **before** multiplying (which also keeps the product inside `u64`), adds `elapsed.as_nanos() as u64 * NS_TO_ACC` to `acc`, then drains whole ticks with a `while acc >= TICK_SCALE` loop, adding to `total`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/tick.rs
git commit -m "$(cat <<'EOF'
Add the fixed 70.086 Hz logic tick

Integer accumulator, so 1000 seconds is exactly 70,086 ticks with no
drift. Elapsed time beyond 250 ms is dropped rather than caught up, so
a suspend or a debugger pause cannot produce a burst of stale frames.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: `IndexedFrame`, banding and compose

**Files:**
- Create: `crates/sr-engine/src/compose.rs`
- Modify: `crates/sr-engine/src/palette.rs` (add `PaletteTimeline`), `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Consumes: `Surface` (Task 1), `Palette` (Task 2), `View` (Task 3)
- Produces: `sr_engine::palette::PaletteTimeline`, `sr_engine::compose::{IndexedFrame, band_range, compose_band}`

```rust
/// Per-frame record of which palette was in force from which scanline.
/// `changes` is kept sorted by scanline and holds at most one entry per scanline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteTimeline { /* base: Palette, changes: Vec<(u16, Palette)> */ }

impl PaletteTimeline {
    pub fn new(base: Palette) -> PaletteTimeline;
    pub fn base(&self) -> &Palette;
    /// Record `pal` as the palette from `scanline` onward, replacing any
    /// existing entry at that exact scanline.
    pub fn push(&mut self, scanline: u16, pal: Palette);
    /// The palette in force at `scanline`: the last change at or before it,
    /// else the base.
    pub fn palette_at(&self, scanline: u16) -> &Palette;
    pub fn change_count(&self) -> usize;
    /// Reset to `base` and drop every change.
    pub fn reset(&mut self, base: Palette);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>,             // len == width * height
    pub palettes: PaletteTimeline,
    pub border: Option<u8>,
}

impl IndexedFrame {
    pub fn new(width: u16, height: u16) -> IndexedFrame;
    pub fn palette_for(&self, scanline: u16) -> &Palette;
    pub fn pixel(&self, x: u16, y: u16) -> u8;
    pub fn row(&self, y: u16) -> &[u8];
}

/// Scanlines `[first, last)` of band `band` of `bands`, partitioning
/// `0..height` exactly for any `height` and any `bands > 0`.
pub fn band_range(height: u16, bands: u16, band: u16) -> (u16, u16);

/// Copy displayed scanlines `[first, last)` from `surface` through `view`
/// into `frame`, sampling the surface as it stands now — this is the beam.
pub fn compose_band(
    frame: &mut IndexedFrame,
    surface: &Surface,
    view: &View,
    first: u16,
    last: u16,
);
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/compose.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Palette;
    use rstest::*;

    fn striped_surface() -> Surface {
        // Each row is filled with its own index, so the source row is readable
        // from any pixel in the composed frame.
        let mut s = Surface::new(8, 4);
        for y in 0..4 { s.row_mut(y).fill(y as u8 + 1); }
        s
    }

    #[test]
    fn band_range_partitions_exactly_for_any_band_count() {
        for &height in &[200u16, 400, 1, 7] {
            for bands in 1..=16u16 {
                let mut cursor = 0u16;
                for b in 0..bands {
                    let (first, last) = band_range(height, bands, b);
                    assert_eq!(first, cursor, "gap or overlap at band {b} of {bands} h{height}");
                    assert!(last >= first);
                    cursor = last;
                }
                assert_eq!(cursor, height, "bands {bands} did not cover height {height}");
            }
        }
    }

    #[rstest]
    #[case(200, 1, 0, (0, 200))]
    #[case(200, 7, 0, (0, 28))]
    #[case(200, 7, 6, (171, 200))]
    #[case(200, 8, 3, (75, 100))]
    #[case(400, 8, 7, (350, 400))]
    fn band_range_splits_evenly_when_height_is_not_divisible(
        #[case] height: u16,
        #[case] bands: u16,
        #[case] band: u16,
        #[case] expected: (u16, u16),
    ) {
        assert_eq!(band_range(height, bands, band), expected);
    }

    #[test]
    fn compose_band_copies_the_right_source_rows() {
        let s = striped_surface();
        let v = View::new(8, 4);
        let mut f = IndexedFrame::new(8, 4);
        compose_band(&mut f, &s, &v, 1, 3);
        assert_eq!(f.row(0), &[0; 8]);
        assert_eq!(f.row(1), &[2; 8]);
        assert_eq!(f.row(2), &[3; 8]);
        assert_eq!(f.row(3), &[0; 8]);
    }

    #[test]
    fn compose_band_uses_the_split_view_for_lower_scanlines() {
        let s = striped_surface();
        let mut v = View::new(8, 4);
        let mut lower = View::new(8, 4);
        lower.set_origin(8); // one surface row down
        v.split = Some(Box::new(Split { scanline: 2, view: lower }));
        let mut f = IndexedFrame::new(8, 4);
        compose_band(&mut f, &s, &v, 0, 4);
        assert_eq!(f.row(0), &[1; 8]);
        assert_eq!(f.row(1), &[2; 8]);
        // Rows 2 and 3 read from surface rows 3 and 0 (wrapped).
        assert_eq!(f.row(2), &[4; 8]);
        assert_eq!(f.row(3), &[1; 8]);
    }

    #[test]
    fn timeline_returns_the_last_change_at_or_before_the_scanline() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        let mut b = Palette::BLACK; b.set_entry(0, [2, 0, 0]);
        tl.push(10, a);
        tl.push(20, b);
        assert_eq!(tl.palette_at(0).entry(0), [0, 0, 0]);
        assert_eq!(tl.palette_at(10).entry(0), [1, 0, 0]);
        assert_eq!(tl.palette_at(19).entry(0), [1, 0, 0]);
        assert_eq!(tl.palette_at(20).entry(0), [2, 0, 0]);
        assert_eq!(tl.palette_at(399).entry(0), [2, 0, 0]);
    }

    #[test]
    fn timeline_replaces_a_change_at_the_same_scanline() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        let mut b = Palette::BLACK; b.set_entry(0, [2, 0, 0]);
        tl.push(10, a);
        tl.push(10, b);
        assert_eq!(tl.change_count(), 1);
        assert_eq!(tl.palette_at(10).entry(0), [2, 0, 0]);
    }

    #[test]
    fn timeline_reset_drops_changes_and_restores_the_base() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        tl.push(10, a);
        tl.reset(Palette::BLACK);
        assert_eq!(tl.change_count(), 0);
        assert_eq!(tl.palette_at(10).entry(0), [0, 0, 0]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine`
Expected: compile error — `IndexedFrame`, `band_range`, `compose_band`, `PaletteTimeline` are not defined.

- [ ] **Step 3: Implement the timeline, `IndexedFrame`, `band_range` and `compose_band`**

`band_range` uses `first = height * band / bands` and `last = height * (band + 1) / bands` in `u32` — that floor-partition covers `0..height` exactly for every `bands`, which is Review Focus #5.

`compose_band` loops `y` in `first..last`, takes `view.effective(y)`, and for each `x` in `0..frame.width` writes `surface.pixels()[v.source_index(surface_len, x, y)]` into `frame.pixels[y * width + x]`. It does **not** touch `frame.palettes` — stamping is the engine's job (Task 6), which is what keeps the "palette in force at the moment the beam reached this scanline" semantics in one place.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/palette.rs crates/sr-engine/src/compose.rs
git commit -m "$(cat <<'EOF'
Add beam compose: IndexedFrame, band partitioning, per-scanline palette

compose_band samples the surface through the view as it stands at the
moment of the call, which is how a mid-frame VRAM update and a mid-frame
palette change land on the same scanline the beam drew them on.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: `Part`, the copper registry, `Engine` and `Runner`

**Files:**
- Create: `crates/sr-engine/src/part.rs`, `crates/sr-engine/src/copper.rs`, `crates/sr-engine/src/engine.rs`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Consumes: `Surface`, `View`, `Palette`, `PaletteTimeline`, `IndexedFrame`, `band_range`, `compose_band`
- Produces: `sr_engine::part::{Part, PartStatus, PartError, PartCtx}`, `sr_engine::copper::{CopperFn, CopperRegistry, COPPER_SLOTS}`, `sr_engine::engine::{Engine, Runner}`

```rust
// copper.rs
pub const COPPER_SLOTS: usize = 3;
pub type CopperFn = Box<dyn for<'c> FnMut(&mut PartCtx<'c>)>;

pub struct CopperRegistry { /* slots: [Option<CopperFn>; COPPER_SLOTS] */ }
impl CopperRegistry {
    pub fn new() -> CopperRegistry;
    pub fn set(&mut self, slot: usize, f: CopperFn);
    pub fn clear(&mut self, slot: usize);
    pub fn is_set(&self, slot: usize) -> bool;
    /// Remove the slot's function so it can be called while the rest of the
    /// engine is borrowed. Always paired with `restore`.
    pub fn take(&mut self, slot: usize) -> Option<CopperFn>;
    pub fn restore(&mut self, slot: usize, f: CopperFn);
}
```

```rust
// part.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartError { pub part: &'static str, pub message: String }
// impl Display + std::error::Error

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartStatus { Continuing, Finished }

pub trait Part {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError>;
    /// Draw one band of this frame, top to bottom. Called `band_count()` times
    /// per logic tick. Returning `Finished` from any band retires the part
    /// when this tick's bands have all run.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus;
    /// 1 (whole frame, composed once) or more (beam compose, composed per band).
    fn band_count(&self) -> u16 { 1 }
}

pub struct PartCtx<'a> {
    pub surface: &'a mut Surface,
    pub view: &'a mut View,
    pub palette: &'a mut Palette,   // the live palette; the engine stamps it per band
    pub frame: u32,                 // logic ticks since this part started
    pub band: u16,
    pub bands: u16,
    pub standalone: bool,
    pub exit_requested: bool,
    // timeline and copper are private; reach them through the methods below
}

impl PartCtx<'_> {
    /// Construct from disjoint borrows of the engine's fields.
    pub fn new<'a>(/* surface, view, palette, timeline, copper, frame, band, bands,
                    standalone, exit_requested */) -> PartCtx<'a>;
    pub fn set_palette(&mut self, pal: Palette);
    pub fn set_palette_entry(&mut self, index: u8, rgb: [u8; 3]);
    /// Register a palette change at an explicit scanline (raster bars).
    pub fn set_palette_at(&mut self, scanline: u16, pal: Palette);
    pub fn set_copper(&mut self, slot: usize, f: CopperFn);
    pub fn copper_is_set(&self, slot: usize) -> bool;
    pub fn request_exit(&mut self);
}
```

```rust
// engine.rs
pub enum EngineError { Run { part: &'static str, message: String } }

pub struct Engine { /* surface, view, palette, timeline, copper, frame_counter */ }

impl Engine {
    pub fn new(surface_width: usize, surface_height: usize, view: View) -> Engine;
    pub fn surface(&self) -> &Surface;
    pub fn surface_mut(&mut self) -> &mut Surface;
    pub fn view(&self) -> &View;
    pub fn view_mut(&mut self) -> &mut View;
    pub fn palette(&self) -> &Palette;
    /// Call `part.init`. Copper slots are cleared first.
    pub fn start(&mut self, part: &mut dyn Part) -> Result<(), EngineError>;
    /// One logic tick, in the fixed order of spec §4.2:
    ///   1. copper slot 0   2. part.frame() per band, composing after each
    ///   3. copper slot 1   4. copper slot 2
    pub fn tick(&mut self, part: &mut dyn Part, out: &mut IndexedFrame) -> PartStatus;
    /// Test-only: register a copper function without going through a `PartCtx`,
    /// which the tests cannot build before `start` has run.
    #[cfg(test)]
    fn set_copper_for_test(&mut self, slot: usize, f: CopperFn);
}

/// Drives an `Engine` over a `Tick`, owning the output frame.
pub struct Runner { /* engine, part, frame, tick, part_frame, status */ }

impl Runner {
    pub fn new(surface_width: usize, surface_height: usize, view: View, part: Box<dyn Part>)
        -> Result<Runner, EngineError>;
    /// One logic tick.
    pub fn step(&mut self) -> PartStatus;
    /// Feed wall-clock time; steps once per due tick, dropping stale frames.
    pub fn advance(&mut self, elapsed: Duration) -> u32;
    pub fn frame(&self) -> &IndexedFrame;
    pub fn status(&self) -> PartStatus;
    /// Logic ticks run since the part started.
    pub fn part_frame(&self) -> u32;
}
```

- [ ] **Step 1: Write the failing tests**

Put the ordering tests in `crates/sr-engine/src/engine.rs`; they need a probe part and shared log.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default)]
    struct Log(Rc<RefCell<Vec<String>>>);

    #[derive(Default)]
    struct ProbePart { log: Log, bands: u16, finish_after: Option<u32> }

    /// Whole-frame part that sets one palette entry and otherwise does nothing.
    struct Flat;

    impl Part for Flat {
        fn name(&self) -> &'static str { "flat" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            ctx.set_palette_entry(0, [40, 0, 0]);
            PartStatus::Continuing
        }
    }

    /// Draws nothing and sets no palette; used with copper callbacks that do.
    struct Noop;

    impl Part for Noop {
        fn name(&self) -> &'static str { "noop" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    /// Reports `Finished` from band 0 of the tick whose `ctx.frame` reaches `n`.
    struct FinishesAt(u32);

    impl Part for FinishesAt {
        fn name(&self) -> &'static str { "finishes-at" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            if ctx.frame >= self.0 { PartStatus::Finished } else { PartStatus::Continuing }
        }
    }

    impl Part for ProbePart {
        fn name(&self) -> &'static str { "probe" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            self.log.0.borrow_mut().push(format!("frame:{}:{}", ctx.frame, ctx.band));
            if let Some(n) = self.finish_after {
                // Reports Finished from the *first* band, so the assertion that
                // every band still ran actually proves something.
                if ctx.frame >= n && ctx.band == 0 { return PartStatus::Finished; }
            }
            PartStatus::Continuing
        }
        fn band_count(&self) -> u16 { self.bands }
    }

    fn copper_logging(log: &Log, slot: usize) -> CopperFn {
        let log = log.clone();
        Box::new(move |_ctx| log.0.borrow_mut().push(format!("copper:{slot}")))
    }

    fn engine_with(part: &mut dyn Part) -> (Engine, IndexedFrame) {
        let mut e = Engine::new(8, 4, View::new(8, 4));
        e.start(part).unwrap();
        (e, IndexedFrame::new(8, 4))
    }

    #[test]
    fn copper_slots_and_frame_calls_run_in_the_fixed_order() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 2, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, copper_logging(&log, 0));
        e.set_copper_for_test(1, copper_logging(&log, 1));
        e.set_copper_for_test(2, copper_logging(&log, 2));
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            ["copper:0", "frame:0:0", "frame:0:1", "copper:1", "copper:2"]
        );
    }

    #[test]
    fn frame_counter_advances_once_per_tick_not_once_per_band() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 4, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            ["frame:0:0", "frame:0:1", "frame:0:2", "frame:0:3",
             "frame:1:0", "frame:1:1", "frame:1:2", "frame:1:3"]
        );
    }

    #[test]
    fn a_finished_band_still_lets_the_tick_finish_its_bands() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 3, finish_after: Some(0), ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        assert_eq!(e.tick(&mut part, &mut f), PartStatus::Finished);
        assert_eq!(log.0.borrow().len(), 3);
    }

    #[test]
    fn a_palette_set_during_a_band_applies_from_that_band() {
        struct Ramp;
        impl Part for Ramp {
            fn name(&self) -> &'static str { "ramp" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                ctx.set_palette_entry(0, [(ctx.band as u8) * 8, 0, 0]);
                PartStatus::Continuing
            }
            fn band_count(&self) -> u16 { 4 }
        }
        let mut part = Ramp;
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        // Four bands over 4 scanlines: one palette change per band.
        assert_eq!(f.palette_for(0).entry(0), [0, 0, 0]);
        assert_eq!(f.palette_for(1).entry(0), [8, 0, 0]);
        assert_eq!(f.palette_for(2).entry(0), [16, 0, 0]);
        assert_eq!(f.palette_for(3).entry(0), [24, 0, 0]);
    }

    #[test]
    fn a_whole_frame_part_stamps_one_palette_for_the_whole_frame() {
        let mut part = Flat;
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        assert_eq!(f.palettes.change_count(), 1);
        assert_eq!(f.palette_for(0).entry(0), [40, 0, 0]);
        assert_eq!(f.palette_for(3).entry(0), [40, 0, 0]);
    }

    #[test]
    fn copper_slot_zero_sees_its_palette_change_on_the_same_frame() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(1, [5, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(1), [5, 0, 0]);
    }

    #[test]
    fn copper_slot_two_runs_after_the_frame_and_affects_the_next_one() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(2, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(2, [9, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(3).entry(2), [0, 0, 0]);
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(2), [9, 0, 0]);
    }

    #[test]
    fn copper_slot_zero_can_record_before_the_part_draws() {
        // Guards the original's use of slot 0 to poll the music mixer: the
        // slot must run before any of the tick's drawing work.
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 1, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, copper_logging(&log, 0));
        e.tick(&mut part, &mut f);
        assert_eq!(log.0.borrow()[0], "copper:0");
    }

    #[test]
    fn runner_presents_the_frame_after_the_tick() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(Flat)).unwrap();
        r.step();
        assert_eq!(r.frame().width, 8);
        assert_eq!(r.part_frame(), 1);
    }

    #[test]
    fn runner_advance_runs_one_step_per_due_tick() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(Flat)).unwrap();
        assert_eq!(r.advance(Duration::from_secs(1)), 70);
    }

    #[test]
    fn runner_stops_stepping_after_the_part_finishes() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(FinishesAt(2))).unwrap();
        r.step(); r.step();
        assert_eq!(r.status(), PartStatus::Continuing);
        assert_eq!(r.step(), PartStatus::Finished);
        let frames_before = r.part_frame();
        assert_eq!(r.advance(Duration::from_secs(1)), 0, "a finished part must not keep ticking");
        assert_eq!(r.part_frame(), frames_before);
    }
}
```

`set_copper_for_test` is a `#[cfg(test)]` helper on `Engine` that registers into a slot without needing a `PartCtx`; the public path is `PartCtx::set_copper` from `init`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine engine`
Expected: compile error — `Engine`, `Part`, `Runner` are not defined.

- [ ] **Step 3: Implement the copper registry, the part trait and the engine**

The tick order in `Engine::tick`, which is the whole point of the module:

```
timeline.reset(palette.clone())
run_copper(0)                      // take slot 0, build a ctx, call, restore
for band in 0..bands:
    status = {
        let mut ctx = PartCtx::new(&mut surface, &mut view, &mut palette,
                                   &mut timeline, &mut copper, frame, band, bands);
        part.frame(&mut ctx)
    }                              // ctx dropped here, so the borrows below are free
    (first, last) = band_range(out.height, bands, band)
    compose_band(out, &surface, &view, first, last)
    timeline.push(first, palette.clone())
run_copper(1)
run_copper(2)
out.border = view.effective(out.height.saturating_sub(1)).border
```

**The ctx is rebuilt for every band, and that is not stylistic.** A `PartCtx` holds `&mut` to the timeline and the palette, so a ctx that stays alive across the loop body blocks the `timeline.push` and `compose_band` calls that must happen between bands. Building one per band, inside a block whose borrows end before the compose, is the shape that compiles. `PartCtx::new` must therefore take the individual field references rather than `&mut self` — disjoint field borrows are fine, a whole-`self` borrow is not.

Slot 0 must run **before** the band loop, so its palette write stamps at scanline 0 via the first band — that is what makes `copper_slot_zero_sees_its_palette_change_on_the_same_frame` pass. Slots 1 and 2 run after the last band, so their writes land in the *next* tick's frame, which is what `copper_slot_two_runs_after_the_frame_and_affects_the_next_one` pins.

Because `CopperFn` is `Box<dyn FnMut(&mut PartCtx)>` and `PartCtx` borrows the engine's fields, `take`/`restore` around the call is what makes the borrow check. Build the `PartCtx` from individual field borrows in one expression (`PartCtx::new(&mut self.surface, &mut self.view, ...)`) — that is a disjoint-field borrow and compiles; a `&mut self` method call would not.

Two signature details worth getting right the first time: `CopperFn` is a higher-ranked type, `Box<dyn for<'c> FnMut(&mut PartCtx<'c>)>` — if closure inference complains, annotate the parameter as `|ctx: &mut PartCtx<'_>|`. And `Engine::start` converts a `PartError` from `part.init` into `EngineError::Run { part: part.name(), message }`, so `EngineError` is the only error type the caller ever sees.

`Runner::advance` calls `Tick::advance`, then `step()` once per due tick, keeping only the last frame. It returns 0 and does nothing once the status is `Finished`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/copper.rs crates/sr-engine/src/part.rs crates/sr-engine/src/engine.rs
git commit -m "$(cat <<'EOF'
Add the Part trait, the three copper slots, Engine and Runner

The copper callbacks the two existing C++ ports stubbed out are
first-class here: slot 0 runs before the frame's drawing, slots 1 and 2
after it, and the palette a part sets during a band is stamped at that
band's first scanline. See decisions.md D6.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Present — indexed frame to RGBA, aspect and border

**Files:**
- Create: `crates/sr-engine/src/present.rs`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Produces: `sr_engine::present::{Aspect, PresentOptions, FrameBuffer, present}`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect { FourThree, SquarePixels }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentOptions {
    pub aspect: Aspect,
    pub scale: u32,     // integer nearest-neighbour factor; >= 1
    pub border: bool,   // draw the overscan colour as a frame
}

impl Default for PresentOptions {
    fn default() -> Self { PresentOptions { aspect: Aspect::FourThree, scale: 1, border: true } }
}

pub struct FrameBuffer { /* width, height, rgba: Vec<u8> */ }
impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> FrameBuffer;
    pub fn width(&self) -> usize;
    pub fn height(&self) -> usize;
    pub fn rgba(&self) -> &[u8];
    pub fn rgba_mut(&mut self) -> &mut [u8];
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4];
}

/// Output size for `frame` under `opts`, in pixels.
pub fn output_size(frame: &IndexedFrame, opts: &PresentOptions) -> (usize, usize);

/// Rasterise `frame` to RGBA: per-scanline palette, nearest-neighbour scale,
/// 4:3 or square pixels, and an optional overscan border.
pub fn present(frame: &IndexedFrame, opts: &PresentOptions, out: &mut FrameBuffer);
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/present.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Palette;
    use rstest::*;

    /// 2x2, row 0 = [1, 2], row 1 = [3, 1].
    /// Scanline 0 uses the base palette; scanline 1 uses one where entry 1 is blue.
    fn two_row_frame() -> IndexedFrame {
        let mut f = IndexedFrame::new(2, 2);
        f.pixels.copy_from_slice(&[1, 2, 3, 1]);

        let mut base = Palette::BLACK;
        base.set_entry(1, [63, 0, 0]); // red
        base.set_entry(2, [0, 63, 0]); // green

        let mut lower = Palette::BLACK;
        lower.set_entry(1, [0, 0, 63]); // blue

        f.palettes = PaletteTimeline::new(base);
        f.palettes.push(1, lower);
        f
    }

    #[test]
    fn square_pixels_scale_is_exact() {
        let f = IndexedFrame::new(320, 200);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: false };
        assert_eq!(output_size(&f, &opts), (640, 400));
    }

    #[rstest]
    #[case(320, 200)]
    #[case(320, 400)]
    #[case(640, 400)]
    fn four_three_output_is_exactly_four_by_three(#[case] w: u16, #[case] h: u16) {
        let f = IndexedFrame::new(w, h);
        let opts = PresentOptions::default();
        let (ow, oh) = output_size(&f, &opts);
        // The 6/5 horizontal stretch must land on whole pixels. It does for
        // every geometry the demo uses; a width that is not a multiple of 5
        // would truncate, which is why the implementation asserts on it.
        assert_eq!(ow, w as usize * 6 / 5, "{w} is not a multiple of 5");
        assert_eq!(ow * 3, oh * 4, "{w}x{h} -> {ow}x{oh}");
    }

    #[test]
    fn four_three_scales_uniformly() {
        let f = IndexedFrame::new(320, 200);
        let opts = PresentOptions { aspect: Aspect::FourThree, scale: 2, border: false };
        assert_eq!(output_size(&f, &opts), (768, 576));
    }

    #[test]
    fn per_scanline_palettes_are_used() {
        let f = two_row_frame();
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 1, border: false };
        present(&f, &opts, &mut out);
        // Row 0 reads the base palette, where entry 2 is green.
        assert_eq!(out.pixel(1, 0), [0, 255, 0, 255]);
        // Row 1 reads the pushed palette, where entry 1 is blue rather than red.
        assert_eq!(out.pixel(1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn nearest_neighbour_replicates_each_source_pixel() {
        let f = two_row_frame();
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 3, border: false };
        present(&f, &opts, &mut out);
        for y in 0..3 { for x in 0..3 { assert_eq!(out.pixel(x, y), [255, 0, 0, 255]); } }
        assert_eq!(out.pixel(3, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn the_border_pads_the_picture_by_eight_source_pixels_scaled() {
        let mut f = two_row_frame();
        f.border = Some(9);
        let mut pal = f.palettes.base().clone();
        pal.set_entry(9, [63, 63, 0]);
        f.palettes.reset(pal);
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: true };
        present(&f, &opts, &mut out);
        assert_eq!(out.width(), 2 * 2 + 2 * 16);
        assert_eq!(out.pixel(0, 0), [255, 255, 0, 255]);
        assert_eq!(out.pixel(16, 16), [255, 0, 0, 255]);
    }

    #[test]
    fn the_border_is_omitted_when_disabled_or_unset() {
        let mut f = two_row_frame();
        f.border = Some(9);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 1, border: false };
        let mut out = FrameBuffer::new(0, 0);
        present(&f, &opts, &mut out);
        assert_eq!(out.width(), 2);
    }
}
```

The tests write `f.pixels` directly — `IndexedFrame`'s fields are public, which is why no accessor is needed. `Palette: Clone` (Task 2) is what lets a test adjust a copy of the base palette.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine present`
Expected: compile error — `present` is not defined.

- [ ] **Step 3: Implement `present`**

`output_size`: `BORDER_PX = 8` per side. For `FourThree`, `w = frame.width * 6 * scale / 5` then `h = w * 3 / 4` (exact 4:3 because 320 and 640 are multiples of 5). For `SquarePixels`, `w = frame.width * scale`, `h = frame.height * scale`. Add `2 * BORDER_PX * scale` to both when `opts.border && frame.border.is_some()`.

`present` resizes `out`, fills it with the border colour if bordered, then for each output pixel maps back to a source pixel with `src_x = (x - border_px) * frame.width / inner_w` and `src_y = (y - border_py) * frame.height / inner_h`, looks up `frame.palettes.palette_at(src_y)`, and writes the RGBA quad from that palette's entry. Aspect is a **display** concern only: `xtask compare` reads `IndexedFrame` directly, so the aspect setting can never affect a fidelity measurement. Record that in the module docs.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/present.rs
git commit -m "$(cat <<'EOF'
Add present: indexed frame to RGBA, 4:3 or square pixels

Aspect and scaling are display concerns only — xtask compare reads the
composed IndexedFrame directly, so no fidelity measurement can depend on
them. The overscan border is padded by 8 source pixels per side; that
padding is the first entry in APPROXIMATIONS.md.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: The synthetic test card

**Files:**
- Create: `crates/sr-engine/src/testcard.rs`
- Create: `docs/fidelity/APPROXIMATIONS.md`
- Modify: `crates/sr-engine/src/lib.rs`

**Interfaces:**
- Consumes: `Part`, `PartCtx`, `Runner`
- Produces: `sr_engine::testcard::TestCard`

```rust
/// A part that cycles through the five visually-significant hardware
/// behaviours of spec §2.4, one phase at a time, so the engine can be
/// exercised end to end with no demo assets.
pub struct TestCard { /* tick: u32 */ }

impl TestCard {
    pub const PHASE_TICKS: u32 = 32;
    pub const PHASE_COUNT: u32 = 5;
    pub fn new() -> TestCard;
    /// 0 bars+per-scanline palette, 1 scroll, 2 split, 3 double-scan,
    /// 4 beam compose + border. Derived from the tick counter.
    pub fn phase(&self) -> u32;
}
```

- [ ] **Step 1: Write the failing tests**

In `crates/sr-engine/src/testcard.rs`, driving a `Runner`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    fn runner() -> Runner {
        Runner::new(MAX_WIDTH, MAX_HEIGHT, View::new(320, 200), Box::new(TestCard::new())).unwrap()
    }

    /// Runs until the part's frame counter reads `phase * PHASE_TICKS + n`.
    /// The counter is read at the *start* of a tick, so reaching tick `target`
    /// takes `target + 1` steps, and `part_frame()` then reads `target + 1`.
    fn frames(phase: u32, n: u32) -> Runner {
        let mut r = runner();
        let target = phase * TestCard::PHASE_TICKS + n;
        for _ in 0..=target { r.step(); }
        assert_eq!(r.part_frame(), target + 1);
        r
    }

    #[rstest]
    #[case(0, 2)]
    #[case(1, 16)]
    #[case(2, 8)]
    #[case(3, 1)]
    #[case(4, 9)]
    fn the_test_card_is_deterministic(#[case] phase: u32, #[case] n: u32) {
        let a = frames(phase, n).frame().clone();
        let b = frames(phase, n).frame().clone();
        assert_eq!(a, b, "phase {phase} tick {n} is not reproducible");
    }

    #[test]
    fn phase_bars_writes_more_than_one_palette_per_frame() {
        assert!(frames(0, 2).frame().palettes.change_count() > 1);
    }

    #[test]
    fn phase_bars_leaves_every_band_a_distinct_palette() {
        let r = frames(0, 2);
        let f = r.frame();
        let mut seen = std::collections::HashSet::new();
        for y in (0..200).step_by(25) { seen.insert(f.palette_for(y).entry(0)); }
        assert_eq!(seen.len(), 8, "expected one distinct palette per band");
    }

    #[test]
    fn phase_scroll_moves_the_view_between_ticks() {
        let a = frames(1, 0).frame().clone();
        let b = frames(1, 1).frame().clone();
        assert_ne!(a, b, "scrolling should change the composed frame");
    }

    #[test]
    fn phase_split_reads_a_different_source_below_the_split_line() {
        let f = frames(2, 8).frame().clone();
        // Surface rows 0..100 hold bars, rows 100..200 hold the inverse. The
        // split view's origin is surface row 100, so displayed row 100 must
        // jump to the inverse pattern.
        assert_eq!(f.pixel(0, 99), 0);
        assert_eq!(f.pixel(0, 100), 15);
    }

    #[test]
    fn phase_double_scan_duplicates_every_source_row() {
        let r = frames(3, 1);
        let f = r.frame();
        for y in 0..200u16 {
            assert_eq!(f.row(2 * y), f.row(2 * y + 1), "row {y} not duplicated");
        }
    }

    #[test]
    fn phase_beam_compose_staircases_the_palette_down_the_frame() {
        let r = frames(4, 2);
        let f = r.frame();
        let top = f.palette_for(0).entry(0);
        let bottom = f.palette_for(199).entry(0);
        assert_ne!(top, bottom, "beam compose should stamp a different palette per band");
        assert!(bottom[0] > top[0], "the staircase should brighten downward");
    }

    #[test]
    fn phase_beam_compose_sets_the_overscan_border() {
        assert!(frames(4, 2).frame().border.is_some());
    }

    #[test]
    fn the_test_card_never_reports_finished() {
        let mut r = runner();
        for _ in 0..(TestCard::PHASE_TICKS * TestCard::PHASE_COUNT + 4) {
            assert_eq!(r.step(), PartStatus::Continuing);
        }
    }
}
```

Every phase must clear the `View` fields it does not own (`split`, `double_scan`, `border`, `x_offset`) before setting its own, because the `View` lives in the `Engine` and persists across ticks and phases. A phase that leaves `split` set makes the next phase's test fail for the wrong reason.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p sr-engine testcard`
Expected: compile error — `TestCard` is not defined.

- [ ] **Step 3: Implement `TestCard`**

`band_count()` returns `8`. `init` sets up a 320×200 view and a 16-colour bar palette. `frame` dispatches on `phase()`:

- **0 bars**: fill the surface with 16 vertical bars (`x / 20`), and set palette entry 0 to `[band * 8, 0, 0]` — one distinct palette per band, which is what `phase_bars_leaves_every_band_a_distinct_palette` reads.
- **1 scroll**: fill with a 1-in-8 vertical stripe pattern from a fixed seed, then `ctx.view.x_offset = (ctx.frame % 320) as u16` each tick.
- **2 split**: draw bars (`x / 20`) into surface rows 0..100 and the inverse (`15 - x / 20`) into rows 100..200, then set `ctx.view.split = Some(Box::new(Split { scanline: 100, view: lower }))` where `lower.set_origin(320 * 100)`. The lower view's origin is advanced by one row each tick so the phase is not static, but the split scanline stays at 100 so the test above can name exact pixels.
- **3 double-scan**: draw a 320×200 pattern, set `ctx.view.double_scan = true`, and `ctx.view.height = 400`.
- **4 beam compose**: draw a vertical ramp by band and set palette entry 0 to `[8 + band * 24, 0, 0]`, monotonic in `band`; set `ctx.view.border = Some(9)` and give entry 9 a visible colour.

Then append the first entry to `docs/fidelity/APPROXIMATIONS.md`:

```markdown
# Approximations register

Every place the port knowingly differs from the original. An entry requires a
justification and a measured visual difference; it is not a parking place for "close
enough".

## A1 — Overscan border is padded, not scanned out

**Part:** all; used deliberately by PANIC and the 320×400 parts.
**Original:** the border is the region of the display outside the active area, set by
AC 0x11 and scanned out by the CRTC.
**Port:** `View::border` is a colour; `present()` draws it as a frame of 8 source pixels
per side, scaled with the picture.
**Why:** the engine composes the active display only (spec §4.2, plan decision 8). The
border must still appear, but its exact thickness depends on CRTC timings the port does
not model.
**Measured difference:** 8 source pixels per side at every geometry. To be re-measured
against the original when PANIC is ported (Phase 5).
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p sr-engine`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/sr-engine/src/lib.rs crates/sr-engine/src/testcard.rs docs/fidelity/APPROXIMATIONS.md
git commit -m "$(cat <<'EOF'
Add the synthetic test card and open the approximations register

The test card cycles through the five behaviours of spec §2.4 — raster
palette, scroll, line-compare split, double-scan, beam compose — with
one property test each.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: `bins/secondreality` — windowed presenter and headless mode

**Files:**
- Create: `bins/secondreality/Cargo.toml`, `bins/secondreality/src/main.rs`, `bins/secondreality/src/cli.rs`, `bins/secondreality/src/headless.rs`, `bins/secondreality/src/window.rs`, `bins/secondreality/tests/headless.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `Runner`, `TestCard`, `PresentOptions`, `Aspect`, `FrameBuffer`, `present`
- Produces: the `secondreality` binary

```
secondreality [--part <name>] [--frames <N>] [--dump-dir <DIR>]
              [--scale <N>] [--aspect 4:3|square] [--no-border] [--headless] [--run-seconds <S>]
```

`--headless` requires `--dump-dir` and either `--frames` or `--run-seconds`.

With `--frames N` it steps the runner exactly N times, writing one `<part>_<index:06>.idx`
(raw indexed, `width * height` bytes) and one `<part>_<index:06>.rgb` (width×height RGB8,
square pixels, scale 1, border off — so a comparison against an emulator frame is 1:1) per
tick, plus a `manifest.json`:

With `--run-seconds S` it drives the runner from real elapsed wall-clock time instead, and
writes only the final frame — ten seconds is 700 ticks, which as 320×200 RGB8 is 130 MB of
output nobody wants. It prints the achieved tick rate, which is the "runs at correct speed"
check.

```json
{ "version": 1, "part": "testcard", "width": 320, "height": 200,
  "frames": [ { "index": 0, "idx": "testcard_000000.idx", "rgb": "testcard_000000.rgb" } ] }
```

- [ ] **Step 1: Write the failing tests**

`bins/secondreality/src/cli.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, String> { Cli::parse(args.iter().map(|s| s.to_string())) }

    #[test]
    fn defaults_to_the_test_card_in_a_window() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.part, "testcard");
        assert!(!c.headless);
        assert_eq!(c.scale, 1);
        assert_eq!(c.aspect, Aspect::FourThree);
        assert!(c.border);
    }

    #[test]
    fn headless_requires_a_dump_directory() {
        assert!(parse(&["--headless", "--frames", "1"]).is_err());
    }

    #[test]
    fn headless_requires_a_frame_budget_or_a_duration() {
        assert!(parse(&["--headless", "--dump-dir", "/tmp/x"]).is_err());
    }

    #[test]
    fn an_unknown_part_is_rejected() {
        assert!(parse(&["--part", "nope"]).is_err());
    }

    #[test]
    fn the_square_pixel_aspect_is_accepted() {
        assert_eq!(parse(&["--aspect", "square"]).unwrap().aspect, Aspect::SquarePixels);
    }
}
```

`bins/secondreality/tests/headless.rs`:

```rust
use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality")).args(args).output().unwrap()
}

#[test]
fn headless_writes_two_frames_and_a_manifest() {
    let dir = std::env::temp_dir().join(format!("sr-headless-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = run(&["--headless", "--part", "testcard", "--frames", "2",
                    "--dump-dir", dir.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["frames"].as_array().unwrap().len(), 2);

    let idx = std::fs::read(dir.join("testcard_000000.idx")).unwrap();
    assert_eq!(idx.len(), 320 * 200);
    let rgb = std::fs::read(dir.join("testcard_000000.rgb")).unwrap();
    assert_eq!(rgb.len(), 320 * 200 * 3);

    let _ = std::fs::remove_dir_all(&dir);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p secondreality`
Expected: compile error — the package does not exist.

- [ ] **Step 3: Implement the binary**

`main.rs` parses `Cli`, then dispatches to `headless::run` or `window::run`. `window::run` initialises SDL2 video, opens a window sized to `output_size(&frame, &opts)`, and each loop iteration calls `runner.advance(frame_start.elapsed())`, re-`present`s the newest frame into the `FrameBuffer`, and uploads it to a streaming texture. The loop uses SDL's frame delay; it **never spins** (spec §5.3). ESC quits.

`headless::run` has two modes. `--frames N` calls `runner.step()` N times, writing `IndexedFrame` and RGB8 output per tick plus the manifest. `--run-seconds S` instead loops until S seconds of **real** wall-clock time have passed, calling `runner.advance(elapsed)` each iteration, writing only the final frame, and printing the achieved tick rate. Because both modes drive the same `Runner` and the same `Tick`, the measured rate is the rate the windowed path runs at too.

Add `bins/secondreality` to the workspace `members`, and set `sdl2 = "0.38"` (its default feature set is empty, so it links the system SDL2 2.30 rather than vendoring), `serde`, `serde_json`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test`
Expected: all tests pass across all packages.

- [ ] **Step 5: Verify the windowed run by hand**

Run: `cargo run -p secondreality` — a window opens, the test card cycles its five phases at 70 Hz, ESC quits.

Run: `cargo run -p secondreality -- --headless --part testcard --run-seconds 10`
Expected: the reported rate is within 0.2 Hz of 70.086. Record the measured number in `PROGRESS.md` in Task 14.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock Cargo.toml bins/secondreality
git commit -m "$(cat <<'EOF'
Add the secondreality binary: SDL2 window and headless frame dump

The engine stays free of SDL2; the binary owns the window. Headless mode
drives the same Runner and dumps native-geometry RGB8 frames so a
comparison against an emulator capture is 1:1.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 10: Vendor the upstream source and the reference binary, and verify the part sequence

**Files:**
- Verify: `reference/original/` (already committed — do not modify it)
- Create: `.gitmodules` (via `git submodule add`), `upstream/SecondReality`
- Create: `docs/fidelity/part-sequence.md`
- Modify: `docs/spec/2026-10-10-second-reality-rust-design.md` §2.2 if the table is wrong

**Interfaces:** none — this task produces documentation and vendored inputs.

- [ ] **Step 1: Verify the reference binary is intact**

```bash
sha256sum reference/original/SECOND.EXE reference/original/REALITY.FC
```

Expected, exactly:

```
a0b637b1c6729aeee8874cb7de701f608fe6ec3028fd72652a962a3a2bd25d8f  reference/original/SECOND.EXE
cffb5839d0660f646c2474ff46452543997c3fc9469fa64401556c2d271aa63b  reference/original/REALITY.FC
```

If either differs, **stop**. Every frame diff in the project is measured against these bytes, and a silent substitution would invalidate all of them. See `reference/original/PROVENANCE.md` and `docs/decisions.md` D12.

- [ ] **Step 2: Add the pinned submodule**

```bash
git submodule add https://github.com/mtuomi/SecondReality.git upstream/SecondReality
git -C upstream/SecondReality checkout 071a82e
```

If GitHub is unreachable, add the local checkout and then set the public URL:

```bash
git submodule add /home/ew/workspace/reality/SecondReality upstream/SecondReality
git -C upstream/SecondReality remote set-url origin https://github.com/mtuomi/SecondReality.git
git -C upstream/SecondReality checkout 071a82e
```

- [ ] **Step 3: Read `MAIN/U2.ASM` and transcribe the part sequence**

Read the part-runner table and the `checkcmdline` routine. For each entry record: order number, part binary name, directory, and the music position it starts at.

- [ ] **Step 4: Write `docs/fidelity/part-sequence.md`**

A table with the columns of spec §2.2, plus a **Source** column citing `MAIN/U2.ASM:<line>`, plus sections for: the music restart points, the `SECOND 1..5` and `SECOND U` block arguments and the `partsmask` bit they map to, and the loop-mode entry point.

Cross-check the block arguments against `reference/original/README.1ST`, which documents them from the distributor's side: `1` = beginning, `2` = title screen, `3` = landscape scroller, `4` = vector part, `5` = end credits. If the assembly disagrees with the shipped readme, the readme describes what users saw and the assembly describes what the code does — record both and say which the port follows.

- [ ] **Step 5: Reconcile against the spec**

Compare with spec §2.2. Every difference is a spec bug — edit §2.2 in place and note the correction in `docs/decisions.md` as D13.

- [ ] **Step 6: Commit**

```bash
git add .gitmodules upstream docs/fidelity/part-sequence.md docs/spec docs/decisions.md
git commit -m "$(cat <<'EOF'
Vendor the upstream source and verify the part sequence against U2.ASM

Spec §2.2 was survey-derived; this replaces it with a transcription of
the runner table in MAIN/U2.ASM with line citations, which the per-part
phases will lean on.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Probe the reference oracle's API **[oracle]**

**Files:**
- Modify: `docs/decisions.md`

**Interfaces:** none — the output is a recorded set of facts that Task 12 implements against.

This task is deliberately exploration, not code. Its deliverable is a `docs/decisions.md` entry describing the exact request and response shapes, so Task 12 can be written without guessing.

**What to boot:** `reference/original/SECOND.EXE`, with `reference/original/` as the working directory so it finds `REALITY.FC` beside it. That is the reference of record (D12) — not `upstream/SecondReality/MAIN/U2.EXE`, which is a developer build. The source-tree shells are only interesting if the distributed release turns out not to run.

- [ ] **Step 1: Start the emulator headless**

```bash
export SR_DOSBOX=/home/ew/workspace/reality/dosbox-automation
export LD_LIBRARY_PATH=$SR_DOSBOX/build/release-linux/_deps/{sdl3,sdl3_image,mt32emu,fluidsynth,zlib-ng,iir1}-build
export DOSBOX_API_TOKEN=$(openssl rand -hex 32)   # must be exactly 64 hex chars
SDL_VIDEODRIVER=dummy "$SR_DOSBOX/build/release-linux/dosbox-automation" \
  --noprimaryconf --set webserver_enabled=true &
echo "token=$DOSBOX_API_TOKEN"
```

`$DOSBOX_API_TOKEN` is 64 hex characters; the emulator rejects any other length and substitutes its own, which the log masks (D11).

- [ ] **Step 2: Enumerate the API and record the shapes**

```bash
curl -s -H "Authorization: Bearer $DOSBOX_API_TOKEN" http://127.0.0.1:8386/api/v1/openapi.json
```

Answer, and write into `docs/decisions.md`:

1. The **image format** of `/api/v1/video/frame` — PNG, BMP, raw RGB, or indexed pixels plus a palette. Record the exact `Content-Type` and the byte layout.
2. Whether an **indexed** capture is available. If it is, `xtask compare` can compare indices instead of RGB and never depend on the DAC widening rule at all. This is the preferred outcome.
3. The **geometry** the frame endpoint returns for a 320×200 Mode X screen, and whether it is stretched horizontally. DOSBox may return the 4:3-corrected width rather than 320.
4. The units and accepted range of **`/api/v1/cpu/cycles`** — whether the value is cycles-per-millisecond, a percentage, or a core multiplier.
5. The request shape of **`/api/v1/script/*`** and `/api/v1/input/sequence`, or whether scripted input is simpler as keyboard events.
6. Whether the framebuffer can be read as **memory**, which would give indexed pixels and the DAC registers directly.
7. **What happens between boot and the first demo frame.** `SECOND.EXE` starts into a text-mode SETUP screen for choosing a sound card and mixing rate. Determine whether passing a block argument (`SECOND 2`) skips it, and if not, what input sequence gets past it. Every capture depends on this, and it is the one thing about the oracle that cannot be worked out from the API document.

Save the literal bytes of the responses you rely on as fixtures, because Task 12's tests parse recorded bytes rather than a live emulator:

```bash
mkdir -p xtask/tests/fixtures
curl -s -H "Authorization: Bearer $DOSBOX_API_TOKEN" \
  http://127.0.0.1:8386/api/v1/dosbox/info -o xtask/tests/fixtures/info.json
curl -s -H "Authorization: Bearer $DOSBOX_API_TOKEN" \
  http://127.0.0.1:8386/api/v1/video/frame -o xtask/tests/fixtures/frame.bin
```

- [ ] **Step 3: Determine the DAC widening rule from the emulator itself**

Boot any original screen with a large flat colour (the setup menu's blue background is enough). Read one pixel's RGB from `/api/v1/video/frame` and its index and palette entry from the memory API. Compute what `dac_expand` must be. If it differs from `(v << 2) | (v >> 4)`, update `dac_expand` in `crates/sr-engine/src/palette.rs` and its test in the same commit.

- [ ] **Step 4: Record the findings and correct any earlier guess**

Append to `docs/decisions.md` as **D14 — The reference oracle's API shape**, with the literal response snippets. If any of Task 12's or Task 13's interfaces in this plan are contradicted by what the API actually returns, say so explicitly in the entry and amend this plan file before continuing.

- [ ] **Step 5: Commit**

```bash
git add docs/decisions.md crates/sr-engine/src/palette.rs
git commit -m "$(cat <<'EOF'
Probe the reference oracle's API and record the response shapes

Recorded rather than inferred, so the capture and compare tooling is
written against the API that exists.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: `xtask` and reference capture **[oracle]**

**Files:**
- Create: `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/oracle/mod.rs`, `xtask/src/oracle/dosbox.rs`, `xtask/src/reference.rs`
- Modify: `Cargo.toml` (workspace members)
- Create: `reference/` (output)

**Interfaces:**
- Consumes: the API facts from Task 11
- Produces: `xtask::oracle::{Oracle, CapturedFrame}`, `cargo xtask reference capture`

```rust
#[derive(Debug)]
pub enum OracleError {
    Unauthorized,
    Http { status: u16, body: String },
    Transport(String),
    /// Capture without pinned CPU cycles is not reproducible; refuse it.
    UnpinnedCycles,
    NotBooted,
    Decode(String),
}
// impl Display + std::error::Error

pub struct CapturedFrame {
    pub width: u16,
    pub height: u16,
    /// Indexed pixels, when the oracle exposes them. Preferred for comparison.
    pub indexed: Option<Vec<u8>>,
    /// Indexed palette as the DAC held it, 0..=63 per component.
    pub palette: Option<[[u8; 3]; 256]>,
    /// Always present.
    pub rgb: Vec<u8>,
}

/// The reference emulator, behind a trait so an unusable oracle degrades to a
/// documented alternative instead of blocking a phase (spec §8.1, D10).
pub trait Oracle {
    fn describe(&self) -> Result<String, OracleError>;
    /// `cwd` matters: SECOND.EXE loads REALITY.FC from the working directory.
    fn boot(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<(), OracleError>;
    fn set_cycles(&mut self, cycles: u32) -> Result<(), OracleError>;
    fn step_frames(&mut self, frames: u32) -> Result<(), OracleError>;
    fn frame(&mut self) -> Result<CapturedFrame, OracleError>;
    fn send_keys(&mut self, script: &str) -> Result<(), OracleError>;
    fn shutdown(&mut self) -> Result<(), OracleError>;
}
```

CLI:

```
cargo xtask reference capture [--exe <PATH>] [--cwd <DIR>] [--arg <A>]... --cycles <N>
                              --frames <I,J,K> --out <DIR> [--skip-boot-frames <N>]
                              [--keys <SCRIPT>]
```

`--exe` defaults to `reference/original/SECOND.EXE` and `--cwd` to `reference/original/`, so
the common case is just `--cycles`, `--frames` and `--out`. `SECOND.EXE` reads `REALITY.FC`
from the working directory, so the two cannot be separated.

Manifest written to `<DIR>/manifest.json`:

```json
{ "version": 1,
  "oracle": "dosbox-automation 0.85.1 (6d100)",
  "upstream_commit": "071a82e",
  "exe": "SECOND.EXE", "arguments": ["2"], "cycles": 3000,
  "geometry": [320, 200],
  "frames": [ { "index": 100, "idx": "frame_000100.idx", "rgb": "frame_000100.rgb",
                "palette": [ [0,0,0], "..." ] } ] }
```

- [ ] **Step 1: Write the failing tests**

In `xtask/src/oracle/dosbox.rs`, against recorded fixtures rather than a live emulator — the REST layer is parsed from bytes, so it is testable without booting anything:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_info_response() {
        let raw = include_str!("../../tests/fixtures/info.json");
        let info = DosboxInfo::from_json(raw).unwrap();
        assert_eq!(info.version, "0.85.1 (6d100)");
        assert!(info.features.cpu_control);
        assert!(info.features.memory);
    }

    #[test]
    fn a_frame_response_yields_rgb_of_the_declared_geometry() {
        let raw = std::fs::read("tests/fixtures/frame.bin").unwrap();
        let f = decode_frame(&raw, "image/bmp", 320, 200).unwrap();
        assert_eq!(f.rgb.len(), 320 * 200 * 3);
    }

    #[test]
    fn an_error_response_is_reported_not_panicked_on() {
        assert!(matches!(
            map_http_error(401, r#"{"error":"unauthorized"}"#),
            OracleError::Unauthorized
        ));
        assert!(matches!(
            map_http_error(500, "boom"),
            OracleError::Http { status: 500, .. }
        ));
    }

    #[test]
    fn the_token_is_exactly_sixty_four_hex_characters() {
        let t = generate_token().unwrap();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
```

Fixture files go in `xtask/tests/fixtures/` and hold **real bytes captured in Task 11**, never invented ones. They are committed, so this task cannot start until Task 11 has written them; if `frame.bin`'s format is not what `decode_frame` was written against, that is Task 11's Step 4 telling you the plan needs amending, not a fixture to hand-edit.

In `xtask/src/reference.rs`:

```rust
#[test]
fn the_manifest_round_trips() {
    let m = CaptureManifest {
        version: 1,
        oracle: "dosbox-automation 0.85.1 (6d100)".into(),
        upstream_commit: "071a82e".into(),
        exe: "SECOND.EXE".into(),
        arguments: vec!["2".into()],
        cycles: 3000,
        geometry: (320, 200),
        frames: vec![FrameRef {
            index: 100,
            idx: Some("frame_000100.idx".into()),
            rgb: "frame_000100.rgb".into(),
        }],
    };
    let json = serde_json::to_string(&m).unwrap();
    assert_eq!(CaptureManifest::from_json(&json).unwrap(), m);
}

#[test]
fn frame_file_names_are_zero_padded_to_six_digits() {
    assert_eq!(frame_file_name(7), "frame_000007");
    assert_eq!(frame_file_name(123456), "frame_123456");
}
```

`CaptureManifest` and `FrameRef` derive `Serialize, Deserialize, Debug, Clone, PartialEq`. The round-trip test is what keeps the on-disk format and the in-memory type from drifting apart, since both `capture` and `compare` read the same file.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask`
Expected: compile error — the package does not exist.

- [ ] **Step 3: Implement the oracle client and `reference capture`**

`xtask/Cargo.toml` depends on `clap` (derive), `serde` (derive), `serde_json`, `ureq` and `png`. `ureq` only ever talks to `http://127.0.0.1:8386`, so no TLS configuration is needed.

`DosboxOracle::spawn` launches the emulator with `LD_LIBRARY_PATH` set from a `SR_DOSBOX` environment variable (defaulting to `/home/ew/workspace/reality/dosbox-automation`), `SDL_VIDEODRIVER=dummy`, and a token it generates itself with 64 hex characters — never scraped from the log, which masks it (D11). Pass `--noprimaryconf --set webserver_enabled=true`.

`capture` boots the DOS image, sends `SECOND.EXE` with the given arguments, pins CPU cycles, runs forward the requested frame indices, and writes `.idx`, `.rgb` and the manifest. **It must pin cycles before the first frame** — a capture without pinned cycles is not reproducible and must be refused with an error rather than silently produced.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask`
Expected: all tests pass.

- [ ] **Step 5: Capture from the original and check determinism**

```bash
CYCLES=<value from Task 11>
KEYS=<the input sequence from Task 11 question 7, if one is needed>
cargo xtask reference capture --arg 2 --keys "$KEYS" \
  --cycles $CYCLES --frames 10,60,120 --out reference/check_a
cargo xtask reference capture --arg 2 --keys "$KEYS" \
  --cycles $CYCLES --frames 10,60,120 --out reference/check_b
diff -r reference/check_a reference/check_b && echo "deterministic"
```

Expected: identical. If the two captures differ, determinism is not achievable at these frame indices and that finding goes in `docs/decisions.md` immediately — every later phase's verification depends on it. Delete `reference/check_b` afterwards; it exists only for this check. Keep `reference/check_a` as the first real capture.

These are frames 10, 60 and 120 from the title screen, not from a part the port renders yet. That is deliberate: Phase 0 verifies the *mechanism* — that the oracle runs, pins cycles, produces frames and repeats itself exactly. Whether those frames match the port is Phase 2's question.

- [ ] **Step 6: Commit**

```bash
git add Cargo.lock Cargo.toml xtask reference/check_a
git commit -m "$(cat <<'EOF'
Add xtask and reference capture

The oracle sits behind a trait, so an unusable emulator degrades to a
documented alternative rather than blocking a phase (decisions.md D10).
Capture refuses to run without pinned CPU cycles, because an unpinned
capture is not reproducible and must never be mistaken for a reference.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 13: `xtask compare` **[oracle]**

**Files:**
- Create: `xtask/src/compare.rs`
- Modify: `xtask/src/main.rs`

**Interfaces:**
- Consumes: the manifest written by Task 12, and the `.idx`/`.rgb` files written by the binary (Task 9)
- Produces:

```rust
#[derive(Debug)]
pub enum CompareError {
    GeometryMismatch { reference: usize, rendered: usize },
    FrameCountMismatch { reference: usize, rendered: usize },
    Io(String),
}
// impl Display + std::error::Error

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameDiff {
    pub differing_pixels: u64,
    pub total_pixels: u64,
    pub differing_percent: f32,
    pub max_channel_delta: u8,
}

/// Both slices must be the same length; a mismatch is a hard error, not a
/// truncated comparison.
pub fn compare_indexed(reference: &[u8], rendered: &[u8]) -> Result<FrameDiff, CompareError>;

/// Frame numbers are paired by their manifest index, not by position in the
/// list, so a capture that skipped a frame cannot silently shift the rest.
pub fn pair_indices(reference: &[u32], rendered: &[u32]) -> Result<Vec<(u32, u32)>, CompareError>;

pub fn verdict(percents: &[f32]) -> &'static str;
```

```
cargo xtask compare --reference <DIR> [--rendered <DIR>] [--run <CMD>] [--out <DIR>]
```

`--run <CMD>` is passed to `sh -c`, which is what lets it be a full command line rather than a single path.

Report written to `<out>/report.json` (default `<reference>/report.json`):

```json
{ "version": 1, "mode": "indexed",
  "frames": [ { "index": 100, "differing_pixels": 0, "total_pixels": 64000,
                "differing_percent": 0.0, "max_channel_delta": 0,
                "diff_image": "diff_000100.png" } ],
  "summary": { "worst_percent": 0.0, "mean_percent": 0.0, "verdict": "identical" } }
```

Comparison is **indexed when both sides have indices**, so the DAC widening rule cannot affect a fidelity measurement, and RGB otherwise. Exit code 0 when every frame is identical, 1 when any differs. `--run` executes a command to produce the rendered frames first.

- [ ] **Step 1: Write the failing tests**

In `xtask/src/compare.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_indexed_frames_report_zero_difference() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 2, 3, 4]).unwrap();
        assert_eq!(r.differing_pixels, 0);
        assert_eq!(r.differing_percent, 0.0);
        assert_eq!(r.max_channel_delta, 0);
        assert_eq!(r.total_pixels, 4);
    }

    #[test]
    fn a_single_differing_index_is_counted_once() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 2, 9, 4]).unwrap();
        assert_eq!(r.differing_pixels, 1);
        assert_eq!(r.max_channel_delta, 7);
        assert_eq!(r.differing_percent, 25.0);
    }

    #[test]
    fn an_index_difference_is_counted_even_when_the_rgb_matches() {
        // Two palette entries may widen to the same RGB. The indices still
        // differ, and the index comparison is the stricter one.
        let r = compare_indexed(&[5], &[6]).unwrap();
        assert_eq!(r.differing_pixels, 1);
    }

    #[test]
    fn mismatched_geometry_is_an_error_not_a_partial_diff() {
        assert!(matches!(
            compare_indexed(&[1, 2, 3, 4], &[1, 2]),
            Err(CompareError::GeometryMismatch { .. })
        ));
    }

    #[test]
    fn frames_are_paired_by_manifest_index_not_by_position() {
        assert_eq!(
            pair_indices(&[10, 20], &[10, 20]).unwrap(),
            vec![(10, 10), (20, 20)]
        );
        assert!(matches!(
            pair_indices(&[10], &[10, 20]),
            Err(CompareError::FrameCountMismatch { .. })
        ));
    }

    #[test]
    fn the_verdict_is_identical_only_when_every_frame_matches() {
        assert_eq!(verdict(&[0.0, 0.0]), "identical");
        assert_eq!(verdict(&[0.0, 0.01]), "different");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p xtask compare`
Expected: compile error — `compare` is not defined.

- [ ] **Step 3: Implement `compare`**

Write `report.json` always, and a PNG diff image per differing frame (differing pixels in magenta over a dimmed copy of the reference) using the `png` crate. Print a one-line summary per frame to stdout. Return exit code 1 when the verdict is `different`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p xtask`
Expected: all tests pass.

- [ ] **Step 5: Verify the mechanism end to end**

```bash
cargo xtask compare --reference reference/check_a --rendered reference/check_a --out out/self
```

Expected: `verdict: identical`, exit code 0.

Then confirm the tool can actually fail, by capturing a second time from a point that cannot match the first:

```bash
cargo xtask reference capture --arg 5 --cycles $CYCLES \
  --frames 10,60,120 --out reference/check_neg
cargo xtask compare --reference reference/check_a --rendered reference/check_neg --out out/neg
```

Expected: `verdict: different`, exit code 1, and a diff PNG on disk. **A compare tool that cannot report a difference is worthless**, so this negative check is part of the deliverable, not optional. Record both results in `PROGRESS.md` and delete `reference/check_neg`.

- [ ] **Step 6: Commit**

```bash
git add xtask/src
git commit -m "$(cat <<'EOF'
Add xtask compare

Compares indexed pixels when both sides expose them, so the DAC
widening rule can never affect a fidelity measurement. Exits non-zero on
any difference so a phase's exit criteria can be checked by a script.

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
```

---

### Task 14: Close out Phase 0

**Files:**
- Modify: `PROGRESS.md`, `docs/decisions.md`, `CLAUDE.md`

- [ ] **Step 1: Run the full suite**

Run: `cargo test --workspace`
Expected: every test passes. Record the counts.

- [ ] **Step 2: Update `PROGRESS.md`**

Mark Phase 0 `done`, fill the Plan column with `docs/plans/00-foundation.md` and the Tag column with `phase-0`. Append a phase-log entry recording:

- what was built,
- the measured tick rate from Task 9 Step 5,
- the oracle determinism result from Task 12 Step 5,
- the compare results from Task 13 Step 5,
- every deviation from this plan and why.

Set "Where to pick up" to Phase 1, naming its first action.

- [ ] **Step 3: Record the plan's decisions in `docs/decisions.md`**

Append D15 covering the ten decisions listed under "Decisions this plan makes that the spec leaves open", with the reason for each. Renumber if an earlier task took a number that was expected to be free — D12 was claimed mid-planning by the reference-of-record decision, so the part-sequence correction is D13 and the oracle API shape is D14.

- [ ] **Step 4: Update `CLAUDE.md`**

Add the `cargo xtask reference capture` and `cargo xtask compare` invocations as they actually work, with the pinned-cycles requirement and the `SR_DOSBOX` environment variable. If Task 11 found a simpler or different path, that is what goes in.

- [ ] **Step 5: Commit and tag**

```bash
git add PROGRESS.md docs/decisions.md CLAUDE.md
git commit -m "$(cat <<'EOF'
Complete Phase 0: foundation and harness

Co-Authored-By: Claude Code <noreply@anthropic.com>
EOF
)"
git tag phase-0
```

---

## Phase 0 exit criteria

From spec §10, checked off at Task 14:

- [ ] `cargo test` green across the workspace.
- [ ] The synthetic test card runs at the correct speed (70.086 Hz measured in Task 9 Step 5) and exercises per-scanline palette, split, scroll, double-scan and beam compose — one property test per behaviour in Task 8.
- [ ] `cargo xtask reference capture` captures frames from the original **[oracle]**.
- [ ] `cargo xtask compare` produces a diff report **[oracle]**.
- [ ] The part sequence in spec §2.2 is verified against `MAIN/U2.ASM` and corrected where it was wrong (Task 10).

Tasks 1–10 have no oracle dependency. If it becomes unavailable, Tasks 11–13 and the last two exit criteria move to Phase 1 and Phase 0 closes as `done (engine only)` with that stated in `PROGRESS.md`.
