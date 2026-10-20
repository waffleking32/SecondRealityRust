# Phase 7 — GLENZ Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port GLENZ — `SECOND 2`'s translucent glenz polyhedron over the FC-logo floor, the red polyhedron that follows it, and the zoom-and-bounce opening that closes BEGLOGO's picture — so the part renders the reference's four keyframes, blocks 1 and 2 run through it to the end of the demo, and the shell gains the block-1 `MUSIC1@0` restart and the post-PANICEND cue wait.

**Architecture:** A new module directory `bins/secondreality/src/parts/glenz/` holding a transliteration of GLENZ's own vector library (the `VID*.ASM` family, *not* Phase 6's VISU): fixed-point rotation/projection (`math.rs` + the source's sine/cosine tables in `tables.rs`), the point-list builder and colour allocator (`poly.rs`), the transparent scanline compositor (`span.rs`), the zoomer/bounce intro and the part driver (`mod.rs`, `intro.rs`). The part keeps the original's 64,000-pixel `vram` and `bgpic` as private linear 320-pixel-row buffers and copies the visible rows into the engine surface each tick; the original's plane masks and the shipped build's 92-byte CRTC row are memory-layout facts the image does not depend on (`reference-indices.md` §7.1, §1.7). The shell gains one restart row (`glenz`, `MUSIC1@0`) and a cue-wait part after `panicend`.

**Tech Stack:** Rust 2021, `crates/sr-engine` (part API, surface/view, copper, palette, music), `crates/sr-assets` (the Uh1 reader already decodes `GLENZ/FC.UH`), `xtask` (reference capture/compare, music trace), SDL2 only in the binary. No new crate and no new dependency.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §10 Phase 7 ("GLENZ verified", `:704`), §5.1's part row, §5.2's generated-loop and self-modifying-code candidates, §12's `dis_getmframe` row. Binding rules: `docs/decisions.md` (D3, D7, D15–D17, D19, D21, D24, D25, D29–D31, D34), `docs/fidelity/APPROXIMATIONS.md`, `CLAUDE.md`, and `docs/fidelity/reference-indices.md` §7.

## Global Constraints

- **The reference is the authority; the shipped release is the tiebreaker.** The vendored tree is a development snapshot (D24). GLENZ has three live D24 questions and each is settled by the reference, never by a fitted constant: the shipped screen's 92-byte CRTC row against the tree's `init320x200` 80 (`VIDINIT.ASM:306` vs `VID.ASM:199-218`), the shipped projection/window constants those rows go with (`projxmul` 256, `projymul` 213, adds 160/130, window 0..319/0..199), and the colour-word key the shipped `demo_glz` allocates slots under. The reference's 2300 bounding box (97..217 × 38..149, centroid 156.8,94.1) is the first check; if a single tree-carriable constant cannot produce it, the divergence is measured and registered.
- **Port the algorithm, not a resemblance.** Fixed-point widths, truncating division, table values and the originals' bugs are transliterated (D7): `rotlist`'s 64-bit `shld` right shifts by 15, `projlist`'s 64/32 `idiv` and 16-bit wrapped centre add, `cliplist`'s 1500 clamp on *y*, the span boundary clamp to **1..319**, the hidden-face colour reduction, the `rolcol`/`rolused` slot pool, and `ceasypolylist`'s carried `bp` that dedupes across polygon boundaries.
- **Generated and self-modifying code.** `ZOOMLOOP.INC` is not linked by GLENZ (`ZOOMER.C`/`ZOOM.ASM` only use `sin1024`); `VEC.ASM`'s matrix-in-immediates and `VIDTWE`/`VIDNRM`'s patched group loops are state-only in the shipped path — the part calls `ceasypolylist` + `cglenz*` + `NEW.ASM`, never `polygroup`/`polyf`. The port keeps the source's structure and lets the compiler hold the matrix.
- **The span compositor's dirty-span walk is an optimisation with identical pixels.** Each span's pixels are `colour | bgpic[pixel]`; a span whose colour did not change already holds exactly that. The port collects the boundaries and repaints every span from `bgpic`, and a test runs the source's two-list walk beside it on a corpus and asserts byte-identical frames. The note records the substitution.
- **Assets are read in their original formats.** `assets/GLENZ/FC.UH` through `sr_assets::load_path` (Uh1, 768-byte palette + 64,000 pixels; `FC.UH` uses indices 0–14). `reference/` and `upstream/` are input-only; the sanctioned writes are `cargo xtask music trace --out reference/music/<name>` and capture output under `out/`.
- **6-bit palette internally**, widened only at upload. The glenz group writes clamp to 63 and select DAC entries 16..255; the per-frame `pal` upload touches **only entries 0..15** (`MAIN.C:414-415`'s `16*3`), so the group entries survive it — `set_palette_entry`, never `set_palette`, for that upload.
- **Never spin-wait.** One logic tick per `dis_waitb`; the intro's waits and the main loop are tick-consuming phases. `ctx.exit_requested` is polled where the source calls `dis_exit`; GLENZ has no timed teardown, so `drains_after_exit` stays false.
- **`sr-engine` and `sr-audio` keep zero runtime dependencies; `sr-assets` may depend on `sr-engine` only.** The renderer is a binary module (D32's precedent).
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29).
- **Tests and clippy are the gate**: `cargo test --workspace` green, zero warnings, both clippy commands clean. Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

Five failure modes the spec implies but no single task's tests would otherwise catch. Each is pinned by a named test in the task that owns the code.

1. **The arithmetic's widths are the whole fidelity.** The rotation is a 64-bit product shifted **right by 15** (`shld ecx,ebx,17` reads as `>>15` of the pair), the projection divides a sign-extended 64-bit product by the 32-bit z, the centre add wraps at 16 bits, and the matrix is signed 16-bit. A cleaner i32 multiply or an untruncated divide moves every vertex at 2300/3000/3400. The maths tests pin hand-computed cases including a negative product and the projection's truncation toward zero.
2. **The span boundary rule is not a conventional rasteriser.** Positions are global `row*320 + x`, x is clamped to **1..319** (not 0), the fill runs to the boundary exclusive, the colour is the XOR of the active boundaries, and every pixel is `colour | bgpic[pixel]`. The clamp alone changes the left column of every polygon that reaches x=0; the OR is what makes overlaps translucent. The span tests pin each and the two-list equivalence test pins the whole compositor.
3. **The colour allocation's two key spaces and one pool.** The visible path indexes `rolcol` by the **full 16-bit colour word**, the hidden path by the **low byte**, and both allocate/free the same even slots 2..30. The port must model that asymmetry (and a pool exhaustion that yields slot 32 → colour 0) or the palette groups land on the wrong faces. The colour tests pin the allocation order and the group ramp; the reference keyframes are the arbiter.
4. **The part's clock is three clocks.** The intro's zoomer is 47 ticks and the bounce 177 (both source-determined), the two `dis_getmframe` waits are driven by MUSIC1's zframe rate, and the main loop advances one frame per tick with its physics, scales and palette phases keyed on that counter. A one-tick error in any of them shifts the moving keyframes. The tick-map tests pin 47/177, the 300/333 waits against a scripted clock, and the palette phase boundaries; the block-2 comparison pins the result.
5. **The shell seams only fire in the right runs.** The block-1 `MUSIC1@0` restart must install only when the run reached it under another module (block 2 already plays MUSIC1@0 and must not reload it); the post-PANICEND cue wait must run after every PANICEND in a plan that continues, wait on `musplus` going **positive**, and consume ticks while holding the screen. The shell tests pin both, and the block-1/2 chain tests pin the numbering.

---

## The measured ground truth

Every row is from the carried Phase 1–6 surveys and this phase's Task 1; **the implementer re-derives each against the file**, because the plan is an argument and the tree is the evidence.

| Item | Source | What it does | Music | Assets | Reference |
|---|---|---|---|---|---|
| GLENZ driver | `GLENZ/MAIN.C` 648 lines, `ZOOMER.C` 141 | `dis_partstart`; wait `musplus < -19`; `setmframe(0)`; `zoomer2()`; mode 13h + `testasm()`; FC-bounce intro; `getmframe<300`, palette build, `getmframe<333`; loop `frame<7000` with the music cut `-16<musplus<0`: rotations +32/+7/0 mod 10800, first object (scale/pos, `cliplist` while `frame<800`), second object after frame 800, `cglenz*`; teardown `setcopper(0,NULL)` | `MUSIC1@0` entry (block 2) or the `:813-818` restart (block 1); the exit cue is `musplus` in (−16,0) | `FC.UH` (Uh1 320×200, palette 6-bit, indices 0–14) | 1499 held (`.rgb`+`.idx`), 2300/3000/3400 moving (`.rgb`); SECOND 2 boot; the collapse begins ~1115 and the floor band at 1499 is the bounce's final frame (rows 153–199) |
| Vector library | `GLENZ/VEC.ASM` 717, `MATH.ASM` 425, `MATHSIN.INC` 45,502 B | `cmatrix_yxz` (arg1→X slot, arg0→Y slot; `sin`/`cos` tables indexed by 0.1°), `csetmatrix` (signed-16 entries), `crotlist` (64-bit sum, `>>15`, +translate), `cprojlist` (y·213/z+130, x·256/z+160, z clamp 128, flags 16/8/4/2/1), `ccliplist` (y≤1500), `ceasypolylist` (addded points, closing dedupe, `checkhiddenbx` cross product, `demomode`), `cglenzinit/done/polylist` → `__newgroup(0/2/1)` | — | — | — |
| Transparent compositor | `GLENZ/NEW.ASM` 418 | `ng_init` double-buffers `newdata1`; group 1 appends edge records (16 bytes: 16.16 x, y1,y2, colour, next, dx) per polygon edge, clipping y1<0, merging duplicate edges by XOR; group 2 walks rows 0–199, sorts each row's active edges by x, emits `(row*320+clamped x, colour)` boundaries, advancing x by dx and clamping to 1..319, then `[63999,0],[-1,0]`; group 3's two-list walk fills changed spans and writes `colour | bgpic[i]` | — | — | the reference's 2300/3000/3400 are the compositor's output over the intro's `bgpic` |
| Colour path | `VEC.ASM:369-553` | `demo_glz`: hidden faces → low-byte slot freed, colour `((word>>1)&1)<<2`; visible → full-word slot from pool 2..30 (32 on exhaustion), shade `clamp(low16(cross>>7))` (or the two-shift form when `lightshift≠9`), group base `slot<<3`, 16 DAC entries `base + Σ backpal[i]>>2`, R=7/9/10, G=shade/2, B=shade (or all-shade); `demo_glz2`: visible keeps the word, hidden → 0/4 | — | `backpal` = `FC.UH`'s palette entries 0–15 (6-bit) | the blue/grey facets at 2300; the red object's palette entries 2/4 after the frame-785 `backpal` rebuild |
| Geometry data | `MAIN.C:38-250` | `points`/`pointsb` (14 vertices each), `epolys`/`epolysb` (24 triangles each, colours 0x4002..0x4030 / alternating 0x4002/0x4004); `points1/7/8/9`, `cubepoints`, `cubeepolys`, `edges`, `polys`, `testlist` are dead in the shipped path (the morph is commented out) | — | — | — |
| The intro | `ZOOMER.C:18-81`, `MAIN.C:288-420` | `zoomer2` 47 ticks: band `zy(n)=Σ⌊k/4⌋`, bottom `125·zy/260`, palette `(pal1·(32−c)+30·c)>>5`, entry 255 bars, final clear and entry-0 black; mode 13h; FC-bounce 177 ticks, final band y1=153/y2=200 (from the source's integer bounce); `tmppal` build; `bgpic` snapshot; `getmframe` waits | reset at the music-wait exit; MUSIC1 zframe rate 0.7422/frame at tempo 130 | `FC.UH` rows | 1499 = the final bounce frame; the collapse band law measured in `reference-indices.md` §3.5; zoomer start (trace zframe reset) ~1112 |
| Shell seams | `MAIN/U2.ASM:801-827`, `:813-818`, `:826-849` | block 2 begins BEGLOGO under MUSIC1@0; block 1 restarts MUSIC1@0 after BEGLOGO and before GLENZ; after PANICEND the shell waits while `musplus ≥ 0`, then until `musplus > 0`, then continues (`@@zh5`/`@@zh4`) | MUSIC1 for blocks 1/2 after the restart | — | the `second2` trace: the post-PANICEND crossing is order 39 row 0 at 8823 (zplus 2), so a positive `musplus` at 8827 |
| The phase's open questions | — | (a) the shipped 92-byte row and its projection constants; (b) the visible-path `rolcol` full-word key; (c) the reference's loop start `L_r` against the port's `L_p` | — | — | Task 1 measures; Task 7 decides with D24/D17 |

---

### Task 1: The measurements and traces Phase 7 needs

**Files:**
- Create: `reference/music/second2b/` (SECOND 2, frames 380–3800: the GLENZ boundary, the intro and the loop's first phase), `reference/music/second1d/` (SECOND 1, the block-1 BEGLOGO→GLENZ boundary; window TBD by the port's chain, ~7800–8800)
- Modify: `docs/fidelity/music-clock.md` (the trace table and the measured positions), `docs/decisions.md` (D36–D38), `docs/fidelity/reference-indices.md` (§7's addendum: the measured intro start, the loop start, the object boxes), `xtask/tests/music.rs`/`verify.rs` if their trace lists are pinned
- Test: `cargo xtask reference verify --tree reference`; `cargo test -p xtask --test music --test verify`

**Interfaces:**
- Consumes: `cargo xtask music trace --arg N --cycles 30000 --frames A..B --out reference/music/<name>`; `cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames … --out out/…`; `cargo xtask music analyse --trace <dir>`.
- Produces: two filed traces; the measured intro start, the loop-start derivation, the object boxes; D36–D38.

**Steps:**

- [ ] **Step 1: Capture the SECOND 2 opening trace.** `cargo xtask music trace --arg 2 --cycles 30000 --frames 380..3800 --out reference/music/second2b`. Record the coverage the manifest states. If a location or re-location refuses, move the window per the R4 precedent and record why; do not cross a module reload.
- [ ] **Step 2: Record what the trace says.** The music position at frame **576** (BEGLOGO's 172 ticks after 404; the port's block-2 GLENZ boundary), the frame where `_np_zframe` **resets to 0** (GLENZ's `dis_setmframe(0)` after the music wait — the zoomer's first tick), and the order/row/zplus at the reset and at 576. In the trace session the reset should be ~1112: `(4700 − 1112) × 0.7422 = 2663`, which is `second2`'s `zframe` at 4700. Write the actual frame, not the estimate.
- [ ] **Step 3: Capture the block-1 boundary trace.** From the port's block-1 chain (PAM's end + BEGLOGO's 172 ticks) name the window; capture `--arg 1 --frames <A>..<B> --out reference/music/second1d`. Record the `MUSIC1@0` restart (the trace's order/row/tempo jump), the zframe reset frame and the positions at the GLENZ boundary. If the window spans a reload and the tool refuses, start after it and record the limit.
- [ ] **Step 4: Probe the reference's opening.** Dense captures in two independent sessions. **`reference capture --frames` takes a comma-separated list, not a range** (`music trace` is the command that takes ranges): build the list with `seq`/`printf`. Capture the collapse's first changing frame (a dense list around 1100–1200, e.g. `1100,1101,…,1200`), the hold and the object's entry (`1250,1300,1350,1400,1450,1499,1550,1600,1650,1700,1750,1800`), and the 2300 screen's stability (`2250,2260,…,2350`). Measure: the first frame that differs from the 1499 hold (the collapse), the first frame carrying **object pixels** above the floor (the pixel-difference against 1499 with the floor rows excluded), and the palette phases by mean brightness. `out/` is scratch; file no new reference frames.
- [ ] **Step 5: Derive the reference's loop start.** `L_r = first_object_index − first_object_tick`, where `first_object_tick` is the first main-loop frame whose projected object draws a pixel, computed from the source's own physics (the implementer reproduces the `ypos`/`jello`/scale recurrence in a scratch script or the Part's unit test and reads it off). Cross-check against the palette transition at `L_r+700…764` (the fade) and `L_r+786` (the `backpal` rebuild): both are visible in the swept frames. Record `L_r` and the session spread.
- [ ] **Step 6: Measure the reference's object boxes** for 2300/3000/3400 above the floor: 2300 is x 97–217, y 38–149, centroid (156.8, 94.1), 9,989 pixels; record 3000's and 3400's the same way. These bound the projection constants and window (a centre of 160/130 fits `init320x200`; 180/175 does not).
- [ ] **Step 7: Record the decisions.**
  - **D36 — the renderer's home and its display model.** `bins/secondreality/src/parts/glenz/` (`math.rs`, `tables.rs`, `poly.rs`, `span.rs`, `intro.rs`, `mod.rs`); the part keeps linear 320-pixel-row `vram`/`bgpic` buffers and copies the view's rows into the engine surface. The original's plane masks and the shipped 92-byte CRTC row are memory-layout facts; the displayed image is 320 pixels a row either way (reference-indices §7.1, §1.7). Cost if wrong: a module move, not a behaviour change.
  - **D37 — the tick model.** The intro is 47 zoomer ticks + 177 bounce ticks + the `getmframe` waits (300 then 333, MUSIC1's zframe rate) + three `dis_waitb` ticks; the main loop advances one frame per tick; the music gates are the boundary wait (`musplus < -19`) and the exit cue (−16 < `musplus` < 0). The measured `L_r` and the trace's reset frame are the calibration.
  - **D38 — the two shell seams.** The block-1 `:813-818` restart is a `MUSIC_RESTART_AT` row `("glenz", Music1, 0)`, installed only when the run's current player is not already MUSIC1@0 (`Sequence::restart_music`'s equality check). The post-PANICEND cue wait is a held part after `panicend` that polls `musplus`, waits while ≥ 0, then until > 0, consumes one tick a call and holds the screen.
- [ ] **Step 8: Commit** the traces, the notes and D36–D38.

**Acceptance:** both traces exist with manifests and pass `reference verify`; `music-clock.md` names them and their measured positions; the intro start, the loop start `L_r` and the object boxes are recorded with the commands that produced them; D36–D38 are written with citations.

---

### Task 2: The sine/cosine tables and the fixed-point maths

**Files:**
- Create: `bins/secondreality/src/parts/glenz/mod.rs` (module skeleton exposing the submodules), `bins/secondreality/src/parts/glenz/tables.rs`, `bins/secondreality/src/parts/glenz/math.rs`
- Modify: `bins/secondreality/src/parts.rs` (module declaration only)
- Test: unit tests in `math.rs` and `tables.rs`

**Interfaces:**
- Produces, consumed by Tasks 3–7:
  - `pub const SIN: [i16; 3600]` and `pub const COS: [i16; 3600]` — `SIN[0..900]` is `MATHSIN.INC`'s `sintable16` quarter, `SIN[900..3600]` is `costable16[0..2700]` (that is the source's layout: `sin(a) = costable16[a−900]`), and `COS` is `costable16` verbatim. A test parses `upstream/SecondReality/GLENZ/MATHSIN.INC` and compares all 3,600 words of each.
  - `pub struct Matrix(pub [i16; 9])`
  - `pub fn matrix_yxz(a0: i16, a1: i16, a2: i16) -> Matrix` — `cmatrix_yxz`'s argument placement: **`a1` fills the matrix's first (X) angle slot**, `a0` the second (Y), `a2` the third (Z), then `calcmatrix`'s `rY·rX·rZ` equations; `checkdeg` wraps each angle into 0..3599 by ±3600; every product is `(s·c) >> 15` (`shld dx,ax,1`), stored as a signed word.
  - `pub fn rotate(dst: &mut [[i32; 3]], count: usize, src: &[[i32; 3]], m: &Matrix, add: [i32; 3])` — or a Vec-returning form; the source's destination accumulates its count and appends at `4 + old*12`, but the shipped calls always start empty. X = `Σ(x·m0 + y·m2 + z·m4) >> 15 + add.x`, Y with m6/m8/m10, Z with m12/m14/m16; the sum is a 64-bit accumulator shifted as `shld ecx,ebx,17`.
  - `pub struct Projection { pub mul_x: i32, pub mul_y: i32, pub add_x: i16, pub add_y: i16, pub min_z: i32, pub min_x: i16, pub max_x: i16, pub min_y: i16, pub max_y: i16 }`; `pub const INIT_320X200: Projection` = 256, 213, 160, 130, 128, 0, 319, 0, 199 (`VID.ASM:199-218`).
  - `pub struct PVert { pub x: i16, pub y: i16, pub flags: u16, pub z: i32 }`; `pub fn project(dst: &mut Vec<PVert>, src: &[[i32; 3]])` — y then x, `(v·mul) as i64 / z_max(128) as i64`, the quotient's low 16 bits wrapped-added to the centre, stored z is the **unclamped** source z, flags: z<128 → 16, y>max → 8, y<min → 4, x>max → 2, x<min → 1.
  - `pub fn clip_y(points: &mut [[i32; 3]], max: i32)` — `cliplist`'s `ds:[si+4] = min(y, 1500)`.
- Tests: the table parse; a matrix at (0,0,0) is the identity in the source's 16-bit scale; a matrix at a known angle matches hand-computed `>>15` values; a negative product's shift; the argument placement (`matrix_yxz(0, 900, 0)` differs from `(900, 0, 0)` in the first slot); rotation of `(5000,0,0)` by the identity reproduces 5000; projection truncates toward zero for a negative y and stores the unclamped z; each flag at its boundary; `clip_y`'s clamp and no-op.

**Steps:**

- [ ] **Step 1: Write the failing table and maths tests** (the parse test reads the vendored `.INC`).
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Transcribe the tables** into `tables.rs` and **implement the maths** in `math.rs`, reproducing the asm's widths. `SIN`'s split layout is the source's, not a mistake.
- [ ] **Step 4: Run and commit.**

**Acceptance:** all 3,600 table words match the source; each shift/truncation quirk has a test that fails on the clean implementation; `INIT_320X200`'s values are pinned.

---

### Task 3: The point-list builder and the colour path

**Files:**
- Create: `bins/secondreality/src/parts/glenz/poly.rs`
- Test: unit tests in `poly.rs`

**Interfaces:**
- Consumes: Task 2's `PVert`, the per-frame `lightshift` and the live palette.
- Produces, consumed by Tasks 4–7:
  - `pub struct PolyDef { pub colour: u16, pub points: &'static [u16] }` — the two shipped tables (`epolys`, `epolysb`) as constants, in the source's order.
  - `pub struct Poly { pub colour: u16, pub points: Vec<(i16, i16)> }`
  - `pub enum DemoMode { Glz, Glz2 }`
  - `pub struct ColourState { pub lightshift: i16, visible: HashMap<u16, u8>, hidden: [u8; 256], used: [bool; 256] }` with `pub fn new() -> Self` (lightshift 9) and `pub fn face(&mut self, colour: u16, cross: i32, mode: DemoMode, palette: &mut Palette) -> u16`.
  - `pub fn build(defs: &[PolyDef], projected: &[PVert], mode: DemoMode, state: &mut ColourState, palette: &mut Palette) -> Vec<Poly>` — `ceasypolylist`: copy points, skipping a repeated previous index (state carried across polygons), drop the closing duplicate, compute `checkhiddenbx`'s 32-bit cross product, call the mode's colour rule, store the count and colour.
  - `pub fn cross(points: &[(i16, i16)]) -> i32` — `(x0−x1)(y0−y2) − (y0−y1)(x0−x2)` in 16-bit multiplies, 32-bit difference; sign negative = hidden.
- The colour rules, transcribed exactly:
  - `DemoMode::Glz` hidden: key the **low byte**, clear its slot and `used[slot]`; output `((colour >> 1) & 1) << 2`.
  - `DemoMode::Glz` visible: key the **full word**; a free even slot from 2..30 (32 if none); store; output `(slot << 3) as u8 as u16`; the shade `s` = signed low-16 of `cross >> 7` (clamp 0..63) when `lightshift == 9`, else the wrapped low-16 sum of `cross>>8` and `cross>>9` (clamp 0..63); R = 7 when `lightshift == 9` else `lightshift`, and for words with bit 1: R that, G = s/2, B = s; without: R=G=B=s; write 16 entries from `slot<<3`, entry j = base + Σ_{i≤j} backpal[i]>>2, each channel clamped to 63.
  - `DemoMode::Glz2`: visible keeps its word; hidden outputs `((word>>1)&1)<<2`. No DAC writes.
- `backpal` is `FC.UH`'s palette entries 0..15, passed in or read once by the part.

**Steps:**

- [ ] **Step 1: Write the failing tests.** The dedupe (a repeated neighbour, a closing duplicate, a repeated vertex across two polygons); `cross` on the three-point cases and its sign; `demo_glz2`'s 2/4 and 0 reductions; `demo_glz`'s slot pool (allocate three faces, hide the middle, reallocate), the full-word vs low-byte keys, the exhaustion case (slot 32 → colour 0); the group's 16 entries for a known shade and `backpal`; the entry index `slot<<3`.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement `poly.rs`**, transliterating `VEC.ASM:260-553` and `MAIN.C:48-240`.
- [ ] **Step 4: Run and commit.**

**Acceptance:** the dedupe, cross, colour and slot tests pass; the visible/hidden key asymmetry and the pool exhaustion are both pinned; the group ramp matches hand arithmetic.

---

### Task 4: The transparent span compositor

**Files:**
- Create: `bins/secondreality/src/parts/glenz/span.rs`
- Test: unit tests in `span.rs`

**Interfaces:**
- Consumes: Task 3's `Poly`.
- Produces, consumed by Task 5:
  - `pub struct Compositor { edges: Vec<Edge> }` with `pub fn clear(&mut self)`, `pub fn add(&mut self, poly: &Poly)` (one call per polygon; `cglenzpolylist`), and `pub fn paint(&self, bgpic: &[u8], pixels: &mut [u8])` (`cglenzdone`).
  - The edge rule: for each consecutive pair (and the closing pair): order the endpoints by y (upper first), skip y1 == y2, clip y1 < 0 (skip y2 ≤ 0; advance x by `slope·(−y1)`), x = x1 in 16.16, dx = `((x2−x1)<<16) / (y2−y1)` truncating toward zero; no merging of coincident edges (XOR is commutative and the pixels are identical).
  - The paint rule: for each edge, for each row y1..y2: position = `y*320 + clamp(x>>16, 1, 319)`, colour = the edge's; advance x by dx. Sort boundaries by position; walk with `colour ^= boundary.colour` and fill `[prev, position)` with `colour | bgpic[i]`; append the sentinels `(63999, 0)` and `(-1)`.
- Tests: a triangle's span bytes; two overlapping polygons' XOR and OR; the clamp at x=0 and x=319; a horizontal edge skipped; an edge wholly above/below the screen; a row-crossing span (a boundary past 319 wraps into the next row by the source's global position); coincident edges; and **`the_full_repaint_equals_the_source_two_list_walk`** — a faithful implementation of `ng_pass2`'s double-buffered walk (the test module's own) against the port's repaint over a corpus of generated frames, byte-identical.

**Steps:**

- [ ] **Step 1: Write the failing span tests** and the two-list reference walk in the test module.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement `span.rs`**, transliterating `NEW.ASM:26-172` (groups 2 and 3) with the D36 repaint substitution; the edge list has no 512 cap (the shipped scenes have ≤ 96 edges; a comment names the original's `MAXLINES`).
- [ ] **Step 4: Run and commit.**

**Acceptance:** the synthetic spans match hand-computed pixels; the clamp, XOR/OR and row-crossing are pinned; the repaint equals the source's two-list walk on the corpus.

---

### Task 5: The intro and the part driver

**Files:**
- Create: `bins/secondreality/src/parts/glenz/intro.rs`, `bins/secondreality/src/parts/glenz/mod.rs` (the `Glenz` part), `bins/secondreality/tests/glenz.rs`, `docs/fidelity/glenz.md`
- Modify: `bins/secondreality/src/parts.rs` (registry row)
- Test: the part's unit tests, `bins/secondreality/tests/glenz.rs`

**Interfaces:**
- Consumes: Tasks 2–4, `PartCtx` (`surface`, `view`, `palette`, `set_palette`, `set_palette_entry`, `music`, `exit_requested`, `music_fresh`), `sr_assets::load_path`.
- Produces: `pub struct Glenz`, registered as `"glenz"`; the phase machine below; the fidelity note.

**The transliteration, from `MAIN.C:273-648` and `ZOOMER.C:18-81` (re-derive each line):**
- **`init`:** read `assets/GLENZ/FC.UH` (320×200; palette entries 0–15 become `backpal`); the inherited view is BEGLOGO's 320×400 (set it if a standalone run enters at 320×200, and note the substitution); the phase is `WaitMusic`.
- **`WaitMusic`:** each tick poll `ctx.music()`'s `musplus()`; continue while `< -19`; on the first tick it is not, `setmframe(0)` and enter `Zoomer`. Exit-aware. No music → `init` returns a `PartError` (the shell refuses a music-gated part in a music-less run).
- **`Zoomer`:** 47 ticks of `zoomer2`: read the live DAC into `pal1`; each tick advance `zya`/`zy` (`zy += zya/4`, cap 260), fill rows `zly..=zy` and `399−zy2..399−zy2+(zy2−zly2)` of the 400-row buffer with 255 (the bottom band starts at `399−zy2` and grows down), `c` = min(frame,32), `pal2[a] = (pal1[a]·(32−c) + 30·c) >> 5` for 0..383, upload entries 0..127; the tick where `zy` is already 260 at the top breaks; then the final clear of all 400 rows and entry 0 → black. No ESC poll (the source's loop has none).
- **`ModeSet`:** one tick: clear the 320×200 buffer (the source's `memset`), set the view to 320×200 (mode 13h: `height=200`, `double_scan=true`, origin 0, `y_line_offset` unset), `testasm()`'s live path installs `INIT_320X200` and the 80-byte rows (state only; the dead second block is recorded, not ported).
- **`Bounce`:** 177 ticks, each drawing the source's band (4 black rows above, the scaled `fcrow` rows, then `fcrow2`'s 16 rows with rows 8–15 black) and clearing rows 0..15 of the DAC to black then entries 0..15 from the picture palette beforehand; the final tick's band is `y1=153, y2=200`; then `Wait300`.
- **`Wait300`:** ticks until `getmframe() >= 300`, then one tick; on that tick build `backpal` (picture palette entries 0–15) and `tmppal` (256 entries: `b = a<16 ? a : a&7`, base `backpal[b]`, +16 on each channel when `a&8` and `a>15`, clamp 63), upload it; `lightshift=9`, positions zeroed; one more tick; snapshot `bgpic` from the 320×200 buffer; enter `Wait333`.
- **`Wait333`:** ticks until `getmframe() >= 333`; then `pal = backpal`, `dis_partstart` (a no-op), one tick, and enter `Main`.
- **`Main`:** each tick, exactly once:
  - If `ctx.exit_requested` or `-16 < musplus() < 0` or `frame >= 7000`: remove nothing (no copper was set), return `Finished` (the screen holds).
  - Upload `pal` to entries 0..15 only; then `frame += 1`; `rx += 32`, `ry += 7`, `rz += 0`, each `%= 10800`; apply the source's physics, scales, positions and palette phases for this frame index (`MAIN.C:423-590`), including the `frame>700` fade/band-clear/`backpal` rebuild, the `frame>800` scale growth, the `frame>2009/2189` shrink, the `bscale>xscale → lightshift=10`, the `frame>2069` fade, and the `frame<800` bounce/jello.
  - `cglenzinit` (compositor.clear); if `xscale > 4`: mode `Glz`, `matrix_yxz(rx,ry,rz)`, rotate `points`, scale matrix (`xscale*64`, `yscale*64`, `zscale*64`) with translate `(oxp, ypos+1500+oyp, zpos+ozp)`, rotate again, `clip_y(1500)` while `frame < 800`, project, `build(epolys, Glz)`, `add` each; if `frame > 800 && bscale > 4`: mode `Glz2`, `matrix_yxz(3600−rx/3, 3600−ry/3, 3600−rz/3)`, rotate `pointsb`, scale `bscale*64`, translate `(oxb, ypos+1500+oyb, zpos+ozb)`, project, `build(epolysb, Glz2)`, add.
  - `cglenzdone`: `paint(bgpic, vram)`; copy the 320×200 buffer's rows into `ctx.surface` (first 320 columns of rows 0..199).
- **Fidelity note:** the tick map (the trace's reset, the 47+177 intro, the waits, `L_r` vs `L_p`), the intro's held screen at 1499, the palette phases, the colour model, the D24 questions, and the reference comparison; every number by a named command or test.

**Steps:**

- [ ] **Step 1: Write the failing unit tests** with a rig that drives the part directly: the zoomer's 47 ticks and final `zy=260`; the bounce's 177 ticks and final band rows 153/200; the final bounce frame's pixels against the picture rows; the wait tick counts under a scripted music at 0.7422/frame; `tmppal`'s entries; the main loop's one frame a tick; the palette phase boundaries (700/764/765/785/790/795/2069/2133); the object on/off rules; the exit cue; ESC finishing without a drain; the first main frame's picture equals `bgpic` (the object is off-screen).
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement `intro.rs` and `mod.rs`**, transliterating `ZOOMER.C` and `MAIN.C`; the waits and loops are phases.
- [ ] **Step 4: Add the registry row** `("glenz", || Box::new(glenz::Glenz::new()))` and register the module.
- [ ] **Step 5: Write `docs/fidelity/glenz.md`** with the measured tick map and the D24 findings.
- [ ] **Step 6: Run the unit tests and commit.**

**Acceptance:** the tick-map tests pass (47, 177, the waits, the one-frame tick); the held intro's pixels match the reference 1499's index plane (the unit test compares the port's buffer to the filed `.idx`); the palette phases and the exit cue are pinned; `--part glenz` runs.

---

### Task 6: The shell seams — the block-1 restart and the post-PANICEND cue wait

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (`music_for_part`, `MUSIC_RESTART_AT`, the cue-wait part and its `Sequence` install, the block-plan/stop and resolve tests), `bins/secondreality/tests/shell.rs` (the block-1/2 chains), `bins/secondreality/src/parts.rs` (no change beyond Task 5), `bins/secondreality/src/cli.rs` and `bins/secondreality/src/window.rs` (the unported-part fixtures move off `glenz`)
- Test: shell unit tests, `tests/shell.rs`

**Interfaces:**
- Produces:
  - `music_for_part("glenz") = Some((ModuleId::Music1, 0))` — the block-2 entry recipe; a `--part glenz` run starts MUSIC1 at order 0 with its own numbering.
  - `MUSIC_RESTART_AT: &[(&str, ModuleId, u8)] = &[("u2e", Music0, 18), ("glenz", Music1, 0)]` — the `:813-818` site. Its guard (`whattorun & 2` and `& 1`) is implied by a plan that reached GLENZ under another module; the equality check keeps block 2 from reloading.
  - **The cue wait:** a `CueWait` part (private to the shell, like `Holding`) with two phases: wait while `musplus() >= 0`; then wait while `musplus() <= 0`; finish on `> 0`. `Sequence::take_next` installs it in place of the next plan part whenever the part that just retired is `panicend` (and the plan has a next part); it draws nothing, holds the surface, consumes a tick per call, and is exit-aware. A run whose plan ends at `panicend` enters the wait and then the hold, as the original does.
  - Block shapes: block 1's plan runs ALKU … GLENZ … ENDSCRL; block 2's runs BEGLOGO … ENDSCRL; neither stops at a part (`stopped_at() == None`), ENDSCRL never retiring. The unported-part fixtures in `cli.rs`/`window.rs`/`shell.rs` use a stub source declining a demo name rather than `glenz`.
- Tests: `the_block_one_run_restarts_music1_at_glenz` (the current module is MUSIC0; a fresh player, order 0, `waiting == 4`); `the_block_two_run_does_not_reload_at_glenz` (the current player is MUSIC1@0; no new install); `the_post_panicend_wait_ends_on_a_positive_musplus` (a scripted music: −3, −1, −32, 0, 1 → five ticks, the last finishing); `the_cue_wait_holds_the_screen` (the surface bytes unchanged while it runs); `blocks_one_and_two_have_no_stop`; `glenz_resolves_under_part`.

**Steps:**

- [ ] **Step 1: Write the failing shell tests** (the restart, the equality skip, the cue wait's phases, the block plans).
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the table rows, the `CueWait` part and the `take_next` install.**
- [ ] **Step 4: Update the fixtures and the real-chain tests.** In `tests/shell.rs`, extend block 1 past PAM into BEGLOGO and to the GLENZ boundary, asserting the restart (MUSIC1@0, waiting 4) and the part switch; extend block 2 to GLENZ and assert the same boundary without a reload. Bound both before GLENZ's intro waits. The post-PANICEND wait's chain test may use a stub plan `[panicend, mntscrl]` and the real player's cue, or the scripted unit test alone; the full block-2 run is not a test.
- [ ] **Step 5: Run `cargo test -p secondreality` and commit.**

**Acceptance:** the restart fires in block 1 and not in block 2; the cue wait's two phases and its screen hold are pinned; blocks 1/2 have no stop; `--part glenz` resolves and runs; all fixtures agree.

---

### Task 7: The block-2 comparison and the registered divergences

**Files:**
- Modify: `bins/secondreality/tests/glenz.rs` (the integration test), `docs/fidelity/glenz.md`, `docs/fidelity/APPROXIMATIONS.md`
- Test: `cargo test -p secondreality --test glenz`; `cargo xtask compare …`

**Interfaces:**
- Consumes: the block-2 chain (Task 6), the part (Task 5).
- Produces: the keyframe comparison and any registered entries (A36, A37).

**Steps:**

- [ ] **Step 1: Run the block-2 dump.** `cargo run --release --bin secondreality -- --block 2 --headless --dump-frames 1499,2299,2300,2301,2999,3000,3001,3399,3400,3401 --dump-dir out/glenz-run` from the repository root. The run must exit 0; record the manifest's indices and the shell's status line.
- [ ] **Step 2: Compare the held opening.** 1499 carries `.idx`: assert **0 differing indices** against `reference/parts/glenz/frame_001499.idx`; the RGB comparison follows from it. If the port's intro hold does not cover 1499 (it must: the hold is ~228 ticks wide and starts ~1336), the tick model is wrong and Task 5 is reopened — do not move the anchor.
- [ ] **Step 3: Compare the moving keyframes.** At 2300/3000/3400 the port's loop frame is `K − L_p`, the reference's is `K − L_r`. If `L_p == L_r`, compare at K; otherwise compare at the port's own phase `K − (L_p − L_r)` and record the offset as **A36** (the session-phase class: D17/D24, like A29/A31). At the aligned phase, the silhouettes must be exact (the object/background split) and any colour residual measured on the facets; the D24 questions (the projection constants, the colour-word key) must be decided by the picture, not by a fitted constant. If a residual remains after the model is settled, register it as **A37** with its pixel count and the diagnosis.
- [ ] **Step 4: Write the integration test** with the length-asserted byte-comparison pattern of `tests/u2a.rs`/`tests/u2e.rs`: the dump helper, the manifest entry, the `0` at 1499, the measured numbers at the moving keyframes with their registered values, and the `--part glenz` refusal/run shape (a standalone run is numbered from zero and says so).
- [ ] **Step 5: Run `cargo test -p secondreality --test glenz` and commit** the test, the note's comparison section and the registrations.

**Acceptance:** 1499 compares at 0 indices; the moving keyframes are compared at a measured, recorded phase; every residual is registered with its count and attribution; no constant was adjusted to chase a number.

---

### Task 8: Close-out — docs, carried debt and the phase record

**Files:**
- Modify: `docs/fidelity/APPROXIMATIONS.md` (A36/A37 and any Task 1 finding), `docs/decisions.md` (finalize D36–D38), `docs/fidelity/reference-indices.md` (§7's addendum), `CLAUDE.md` (twenty-one parts, the block examples, the GLENZ ESC/cue-wait note), `PROGRESS.md` (Phase 7 done, the evidence), `docs/fidelity/asset-inventory.md` if the FC.UH row needs the reader note, `docs/fidelity/bugs.md` if the preserved quirks are listed there
- Test: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` (both the default and `--release` invocations)

**Steps:**

- [ ] **Step 1: Write the remaining fidelity-note sections** and the approximation entries with their measured differences (the span repaint substitution if not registered sooner, the shipped-layout findings, A36/A37).
- [ ] **Step 2: Finalize D36–D38** with the measured values (the trace's reset, `L_r`, the keyframe offsets).
- [ ] **Step 3: Update `CLAUDE.md`.** The part count becomes twenty-one; the run examples: block 1 runs ALKU → … → GLENZ → … → ENDSCRL and has no stop; block 2 runs BEGLOGO → … → ENDSCRL; block 3/4 unchanged; the ESC paragraph names GLENZ's music-cut exit and the post-PANICEND cue wait.
- [ ] **Step 4: Update `PROGRESS.md`.** Phase 7's row is **done** with the tag withheld until the final review (the project's practice), the evidence: the two traces, the four keyframe comparisons, the shell seams, the new decisions and approximation numbers.
- [ ] **Step 5: Run the full suite and both clippy gates; commit.**

**Acceptance:** every note's numbers are reproducible by a named command or a same-commit test; `CLAUDE.md` and `PROGRESS.md` match the runnable truth.

---

## Self-review notes (for the controller, not the implementers)

- **Spec coverage:** §10's "GLENZ verified" → Tasks 5–7; §5.2's generated-loop/self-modifying-code rows → the Global Constraints and Task 5's note; §12's `dis_getmframe` risk → Task 1's trace and Task 5's waits; the shell's `:813-818` and `@@zh5` sites → Task 6.
- **Task boundaries:** Tasks 2–4 are the library's three independently reviewable layers (maths, point/colour, compositor), each consumed through a named interface; Task 5 is the part; Task 6 the shell; Task 1 measurement; Task 7 the comparison; Task 8 the record.
- **Known coupling:** the keyframe comparison needs the shell's block-2 numbering (Task 6), which is why the integration test is Task 7 and not part of Task 5; the part's unit test can compare its buffer to the filed 1499 `.idx` without the shell.
- **The plan's open measurements:** (a) the shipped 92-byte row / projection constants; (b) the visible-path full-word colour key; (c) `L_r` vs `L_p`. Each is measured in Task 1 or decided by the Task 7 comparison under D24, and each has a named fallback registration rather than a fitted constant.
