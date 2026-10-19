# Phase 6 — Vector engine (U2A and U2E) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the VISU 3D engine and the two vector parts — U2A (the landing pad and flying saucer over the U2ABG landscape) and U2E (the city flythrough) — so each renders the reference's own keyframes, each waits on the music exactly as the original does, block 1 runs through U2A and PAM, block 4 runs for the first time, and block 3 crosses the mid-run `MUSIC0@18` restart before U2E.

**Architecture:** A shared renderer module under `bins/secondreality/src/parts/visu/` (fixed-point matrix/projection maths, the polygon pipeline, and the scene driver), with the packed scene formats read as plain data by a new `crates/sr-assets/src/visu.rs`. The two parts are transliterations of `VISU/C/U2A.C` and `VISU/C/U2E.C` over that engine, using four 64,000-pixel pages laid into the engine surface and flipped through copper slot 2 exactly as the original flips CRTC 0x0c. The shell gains the restart site at the U2E boundary (`MAIN/U2.ASM:880-885`) and U2E's measured first visible frame, from which its run anchor is derived.

**Tech Stack:** Rust 2021, `crates/sr-engine` (part API, surface/view, copper, palette, music), `crates/sr-assets` (asset readers), `xtask` (reference capture/compare, music trace), SDL2 only in the binary.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §4.4 (the deferred packed scene/object format, `:383-385`), §10 Phase 6 ("U2A and U2E verified", `:703`), and §12's vector-engine risk row. Binding rules: `docs/decisions.md` (D3, D7, D15–D17, D19, D21, D24, D25, D29, D30, D31), `docs/fidelity/APPROXIMATIONS.md`, and `CLAUDE.md`.

## Global Constraints

- **The reference is the authority; the object and the shipped release are the tiebreakers.** The vendored tree is a development snapshot (D24). U2A is the phase's live case: the tree's `U2A.C` cannot draw before its music gate, yet the reference holds a U2ABG screen from ~5960, and the release's U2A is compressed. Where a faithful tree port disagrees with a reference frame, D24's procedure decides, and the divergence is recorded.
- **Port the algorithm, not a resemblance.** Fixed-point arithmetic, table values, rounding and the originals' *bugs* are transliterated (D7): `mulmatrices`' low-32-bit accumulation versus `rotatesingle`'s 64-bit `shrd`, the signed-16-bit matrix reads, `calc_project`'s truncation and per-vertex flags, the near-clip Gouraud colour copy, U2A's uninitialised first-frame `fov`, U2E's skipped first draw and its 768-entry fade from index 255.
- **Assets are read in their original formats.** No conversion. `reference/` and `upstream/` are input-only; the one sanctioned write is `cargo xtask music trace --out reference/music/<name>` and capture output under `reference/parts/`.
- **6-bit palette internally**, widened only at upload. `.00M`'s palette is 6-bit and is copied with `Palette::set_entry`; `Palette::from_vga` would shift it down by two and is wrong here.
- **Never spin-wait.** The logic tick is fixed at 70.086 Hz. Every original `for(;;)` wait and every `dis_waitb` is a tick-consuming phase that returns `Continuing`; a part returns `Finished` only when its stream says so (or its exit gate fires). `ctx.exit_requested` is polled wherever the source calls `dis_exit`.
- **The view's semantics are D16's**: `start_addr` is a pixel address and `y_line_offset` is the row stride. U2A/U2E's four 64,000-pixel pages occupy the surface at origins 0, 64,000, 128,000 and 192,000 with `y_line_offset = 320`; CRTC 0x0c's byte address `n*16384` converts to pixel origin `n*64000`.
- **`sr-engine` and `sr-audio` keep zero runtime dependencies; `sr-assets` may depend on `sr-engine` only.** No new crate: the renderer is a binary module, like `parts/poslist.rs` (D32 below).
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29).
- **The mid-chain rule stays binding** (D31): a part's entry pre-roll runs only when `ctx.music_fresh()`. U2A and U2E gate on absolute music positions, so they wait in both cases; they must not *skip* a wait when fresh.
- **Tests and clippy are the gate**: `cargo test --workspace` green, zero warnings, both clippy commands clean. Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

Five failure modes the spec implies but no single task's tests would otherwise catch. Each is pinned by a named test in the task that owns the code.

1. **The animation stream's frame boundaries must survive every opcode.** `0xFF` followed by a byte >0x7F falls through to the command decoder; the `0xC0` prefix's second byte carries the toggle and pflag bits; frame deltas are signed and added. A parser that treats any `0xFF` as a frame end mis-counts U2A (522 frames) and U2E (1,802). The reader's test decodes both shipped streams and asserts the exact frame/command counts, the first and last frames, and every visibility toggle.
2. **The fixed-point maths must reproduce two different multiply paths and the signed-16-bit matrix reads.** `mulmatrices` keeps the low 32 bits then `sar 14`; `rotatesingle` uses the full 64-bit `shrd`; matrix entries are read as signed words; projection divides with truncation and stores 16 bits. A cleaner 64-bit multiply in the wrong place moves polygon vertices at the keyframes. The maths tests pin hand-computed cases for each path.
3. **Clipping can exceed the original's 16-side buffers.** Shipped geometry reaches 12 sides; a 12-gon through five clip planes produces more. The port must handle more without changing the shipped-data results. The pipeline test clips a 12-gon through all five planes and asserts the output remains a simple polygon and equals the original's algorithm on cases that fit.
4. **The page ring's timing determines which picture each index shows.** U2A displays one page per retrace, U2E two retraces per animation frame, U2E skips its first draw, and the copper runs after the compose. One tick of flip error shifts every moving keyframe. The driver tests pin the ring state machine for both cadences and the display origin at each tick; the part integration tests compare moving keyframes with the D19 supports.
5. **The U2ABG payload is already a linear 320×200 image; the source's de-interleave is a planar-VGA artifact.** Using `origin`'s payload as-is is correct for the port; applying the planar de-interleave again would scramble it (and vice versa). The U2A test pins the first row's pixels against the Uh payload, and the note says why the de-interleave is not ported.

---

## The measured ground truth

Every row is from the five Phase 6 surveys; **the implementer re-derives each against the file**, because the plan is an argument and the tree is the evidence.

| Item | Source | What it does | Music | Assets | Reference |
|---|---|---|---|---|---|
| U2A | `VISU/C/U2A.C` 515 lines over `VISU.LIB` + `_BG.OBK` | load `.00M`/3 objects/`.0AA`/`.0AB`; patch palette 192–255 from the background; verify the environment; gate 1; `vid_init(3)` (no tweak), palette, `vid_window(0,319,25,174,512,9999999)`, copper slot 2, partstart, 4-page ring, 1 stream frame per retrace, gate 2; teardown clears 4 pages with bg | raw `int 0fch` fn 6: gate 1 `order>10 && row>46` (first satisfiable at order 11 row 47 ≈ 6985–6990); gate 2 `order>11 && row>54` (first at 7618, normally not reached) | `U2A.00M` 796 B, conum 6 (`1,2,3,1,1`); `.001/.002/.003` (159/285/45 verts); `.0AA` 8 B; `.0AB` 16,081 B, **522 frames**, 1,272 commands, fov 0x22; `U2ABG.UH` 64,784 B (Uh 320×200, 768-B palette, 64,000 linear pixels) | 6500 (pad, held, RGB only), 7300 (saucer, moving, RGB+idx), 7640 (plain, held, RGB+idx); block 1, SECOND 1; gate 1 measured at **6988** (11:47, `second1c`; the port's block-1 clock matches `second1`'s phase and opens at 6986 — the traces differ by ≈3 zframes at 5900, D17's session spread), gate 2 at 7618; drawing ≈6988–7510; the pre-gate pad is U2ABG under the release's bright palette `(3×bg+5)&63`, entry 195 black, painted at U2A's start |
| U2E | `VISU/C/U2E.C` 628 lines over `VISU.LIB` | load `.00M`/42 objects/`.0AA`/`.0AB`; jelly check; intro: 33-step fade-in, 16 waits, `fadeset`×4 + CRTC 9 \|=0x80, 16 waits, 33-step fade-out; `vid_init(11)`, palette overrides 252–255, window, copper, partstart; gate `order>18`; 4-page ring, first draw skipped, 2 retraces per stream frame, `currframe` hacks; teardown 16-step DAC fade from index 255 | raw `int 0fch` fn 6: `order>18` | `U2E.00M` 900 B, conum 58 (`1..42` + duplicates `9,9,9,9,6,25,32,32,35,35,32,32,32,4,32`); `.001` (29 verts); `.0AB` 77,069 B, **1,802 frames**, 4,047 commands, fov 0x1C | 1000/1600/2600/3600 (all moving, RGB only), SECOND 4 boot; boot 420 loader black, 500 fade, 600=700 hold, 800 corridor, 1000 console; first visible frame **467** (the fadeset), so the run anchor is the intro's start, ≈467 − 49 = 418 |
| Packed formats | `VISU/C/SAVE.C`, `VISU/VISU.C`, `VISU/C/MAIN.C`, `VISU/CD.H`, `VISU/AD.H` | section = 4-char label + LE32 length (padding included), 4-aligned; `VERS`/`NAME`/`VERT`/`NORM`/`POLY`/`ORD0`/`ORDE`/`END `; face record = `sides\|(flags&0xFF00)`, color, normal, sides×u16 (6+2·sides); lists = count, center, offsets, 0 | — | all present under `assets/VISU/C/SCENE/`; no converter work | — |
| Engine maths | `VISU/ACALC.ASM`, `ADRAW.ASM`, `ADRAWCLP.ASM`, `AVIDFILL.ASM`, `ADATA.ASM`, `AVISTAN.INC` | `UNIT=16384`; `calc_applyrmatrix` = apply×dest with position rotate+translate; `calc_rotate` signed-16 matrix reads, 64-bit shrd 14; `calc_nrotate`; `calc_project` (z clip 512..1e9, y then x, truncating divide, 16-bit store, vf AND); `calc_singlez`; camera angle: the asm halves the stream's full angle before the table (U2A 360/316, U2E 446/391, aspect 225); window centre (159,99); painter sort far-to-near; face lists pre-sorted, list chosen by minimum sort-vertex z; culling by normal·vertex sign; light `(12118,10603,3030)`; `normallight` >>5+128 clamp; `calclight` 1..30 | — | — | — |

---

### Task 1: The reference measurements and traces Phase 6 needs

**Files:**
- Create: `reference/music/second1c/` (SECOND 1, frames 5900–7600: U2A's wait and drawing), `reference/music/second4/` (SECOND 4, frames 500–4400: the block-4 restart and U2E's gate), `reference/music/second3e/` (SECOND 3, frames 18000–22000: the mid-run restart and U2E's opening)
- Modify: `docs/fidelity/music-clock.md` (the trace table and the measured orders), `docs/decisions.md` (D32–D34 below), `xtask/tests/music.rs` and `xtask/tests/verify.rs` (explicit trace lists/floors), `crates/sr-audio/tests/trace.rs` if its trace set is explicit, `docs/fidelity/reference-indices.md` only if a new measured start index is found for U2E
- Test: `cargo xtask reference verify --tree reference`; `cargo test -p xtask --test music --test verify`; `cargo test -p sr-audio --test trace`

**Interfaces:**
- Consumes: `cargo xtask music trace --arg N --cycles 30000 --frames A..B --out reference/music/<name>` (the Phase 5 recipe), `cargo xtask music analyse --trace <dir>`, `cargo xtask reference capture --arg 4 … --out out/…` for probes.
- Produces: three filed traces with manifests; the measured music positions named below; the U2E start anchor; the U2A pad's palette; decisions D32–D34.

**Steps:**

- [ ] **Step 1: Capture the three traces.** `second1c`: `--arg 1 --frames 5900..7600`. `second4`: `--arg 4 --frames 500..4400`. `second3e`: `--arg 3 --frames 18000..22000`. Each runs the tool's own base location at the trace start and re-location at the end; all three start *after* a `MUSIC0@18` restart or in a session where none intervenes, so the fixed base is valid. If a locate or the end re-locate refuses, move the start later by one row and record why in `music-clock.md`; do not widen a window across a reload (`second3d`'s R4 precedent).
- [ ] **Step 2: Record the measured positions.** From `second1c`: U2A gate 1's first satisfying frame (order 11 row 47) and gate 2's (order 12 row 55, expected 7618 in `second1b`). From `second4`: the order-19 frame (U2E's gate), the order:row at 1000/1600/2600/3600, and the row cadence. From `second3e`: the order:row immediately after the restart and at 18000/18100, compared with `second4`'s opening hold (they are byte-identical screens, §18.1). Write each into `music-clock.md` with the trace name and a `cargo xtask music analyse` citation.
- [ ] **Step 3: Measure U2E's start.** Probe with `cargo xtask reference capture --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 420,430,440,450,460,470,480,490,500 --out out/u2e-start`, then a dense `456..473` scan in two independent sessions. **Host limit:** the capture's demo-start detection returns at 419–420, so 404/410 are refused ("reached frame 419/420 before frame 404 could be captured"); start at 420 and record the limit. **Measured:** 456–466 are the loader's flat black `ea0787f6…`; 467 onward is the fadeset band `6eb4e4cd…`; both dense sessions agree exactly. **467 is the first *visible* frame (the fadeset), not the run's first tick:** `U2E.C:285-335` runs 33 fade-in waits and 16 further waits before `fadeset` writes the displayed page, so the part's tick 0 is ~49 ticks earlier. The run anchor is that intro start, derived as 467 − 49 = **418** (the candidates 417/418 differ by the off-by-one in whether the fadeset's own wait is counted before or after the write); Task 7's tick-map test calibrates it so the fadeset lands at 467. File no new reference frames: this is a probe, and `out/` is scratch.
- [ ] **Step 4: Resolve the U2A pad — measured, and it is neither of the plan's candidates.** `reference/parts/u2a/frame_006500.rgb` is the U2ABG index image under a different DAC palette (survey §3). Grouping its RGB by `frame_007640.idx` gives 52 consistent entries with zero conflicts; `frame_007640`'s own palette inverts with the emulator's `round(v×255/63)` (0 mismatches over 64,000 pixels; the project's `(v<<2)|(v>>4)` does not). **Result:** the 6500 DAC is `(3 × U2ABG palette + 5) & 63`, with entry 195 black — 57,516 of the 57,600 pixels that use indices ≥192 match; the 84 index-195 pixels render black. An independent check from 7300's own RGB by its own index plane confirms the formula (0 conflicts; its derived palette equals its manifest, and entries 192–255 are U2ABG 0–63 at all 51 used indices). Candidates (a) the merged scene palette and (b) the ALKU port's final palette each match **0/64,000**. Captures at 5960/6000 show 5960 = the natural background (byte-identical to 7640) and 6000 = the bright pad, and the ALKU-end screens (5859/5900/5940) are under the normal palette: the release paints the background and uploads the pad palette at U2A's start, in (5960, 5966], and the pad is U2A's own. Task 6 implements this; the ledger carries the measured table.
- [ ] **Step 5: Record the decisions.**
  - **D32 — the renderer's home.** `bins/secondreality/src/parts/visu/` (a module directory: `math.rs`, `draw.rs`, `mod.rs`), not a new crate; the packed-format readers live in `crates/sr-assets/src/visu.rs` as plain data. Rationale: plan 02's no-new-crate stance, the `parts/poslist.rs` precedent for code shared by two parts, and D3/D16 keeping demo-specific 3D out of `sr-engine`; the cost if wrong is moving one module behind a crate boundary.
  - **D33 — the tick models.** U2A advances one stream frame per logic tick and flips one page per tick; its drawing begins at gate 1 (measured frame **6988**, order 11 row 47) and gate 2 holds at 7618 (order 12 row 55). U2E advances one stream frame per **two** logic ticks (`currframe += repeat*2`) and skips its first draw; its intro is 100 `dis_waitb` ticks (33 fade-in + 16 + 2 around the `fadeset`/CRTC writes + 16 + 33 fade-out); its drawing begins at gate `order>18`, measured at frame **699** (order 19 row 0) in `second4`. **U2E's run anchor is the intro's start, not the first visible frame:** the measured first visible frame is **467** (the fadeset, after 49 pre-visible intro ticks), so `boot_anchor("u2e")` is the derived **418** (candidate; calibrated by Task 7's tick-map test so the fadeset lands at 467).
  - **D34 — the mid-run restart model.** At the U2E boundary the shell installs a fresh MUSIC0 seeked to order 18 (the row for block '4', whose guard `whattorun & 8` is implied by U2E's presence in any plan). The new player waits `MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME = 4` engine ticks before advancing. **Verified by `second3e`:** `second4` and `second3e` are the same restarted session shifted by 17,441–17,442 frames (17,442 at the measured rows; the ±1 is row sampling); the restarted clock starts at 17,850.2, the U2E boundary at 17,846.2 = chain 17,427, and the first music tick at chain 17,431 = boot 17,850 — the 4-tick wait. The restart does **not** zero `_np_zframe` (9,099 at 18000, carried from PLZPART's 5,723 reset); `music-clock.md` §2's causal clause is corrected.
- [ ] **Step 6: Commit.** `git add reference/music/second1c reference/music/second4 reference/music/second3e docs/fidelity/music-clock.md docs/decisions.md docs/fidelity/reference-indices.md` (whatever changed) and commit.

**Acceptance:** the three traces exist with manifests and pass `reference verify`; `music-clock.md` names them with their windows and the measured positions; the U2E first visible frame is measured in two sessions and the run anchor derived from it is recorded; the pad palette question is answered against the two candidate palettes; D32–D34 are written with citations.

---

### Task 2: The packed scene readers

**Files:**
- Create: `crates/sr-assets/src/visu.rs`, `crates/sr-assets/tests/visu.rs`
- Modify: `crates/sr-assets/src/lib.rs` (expose `pub mod visu;`), `xtask/src/assets.rs` or the verify path that counts decoded assets, `xtask/tests/assets.rs`, `xtask/tests/verify.rs` (pinned counts), `docs/fidelity/asset-inventory.md` if the rows need the new decoder note
- Test: `crates/sr-assets/tests/visu.rs`, `cargo test -p xtask --test assets --test verify`, `cargo test -p sr-assets`

**Interfaces:**
- Produces (in `sr_assets::visu`), consumed by Tasks 5–7:
  - `pub struct Master { pub city: u8, pub palette: Palette, pub objects: Vec<u16> }` and `pub fn read_master(bytes: &[u8]) -> Result<Master, AssetError>` — `city` is byte 15 (`b'C'`→1, `b'R'`→2, else 0); `objects` is the `conum-1` file numbers; palette entries are 6-bit and copied with `Palette::set_entry`.
  - `pub struct SceneList { pub scenes: Vec<u16> }` and `pub fn read_scene_list(bytes: &[u8]) -> Result<SceneList, AssetError>` — u16 pairs, stop at a zero first word or `0xFFFF`; `scenes` excludes the sentinel.
  - `pub struct Animation { pub frames: Vec<Frame> }`, `pub struct Frame { pub fov: u8, pub ops: Vec<Op>, pub end: bool }`, `pub struct Op { pub object: usize, pub toggle: Option<bool>, pub dx: i32, pub dy: i32, pub dz: i32, pub dm: [Option<i32>; 9] }`; `pub fn read_animation(bytes: &[u8], conum: usize) -> Result<Animation, AssetError>` — each frame's terminator `0xFF b` sets that frame's `fov = b` and ends it; `0xFF 0xFF` ends the stream; an object index `>= conum` is an error; deltas are signed and added by the caller; `dm[b] = Some(delta)` for each matrix entry the pflag selects, in file order. **The END frame is a frame with `end: true`, no ops and `fov = 0`** — an empty `ops` is *not* a terminator (commandless frames are legitimate: U2A frames 1–15, 30 U2E frames); consumers treat `end` as the stream's terminator and reconstruct the drawable angle as `u16::from(fov) << 8`.
  - `pub struct ObjectData { pub version: u16, pub name: Vec<u8>, pub vertices: Vec<Vertex>, pub normals: Vec<Normal>, pub polys: Vec<Face>, pub lists: Vec<FaceList> }`, `pub struct Vertex { pub x: i32, pub y: i32, pub z: i32, pub normal: i16 }`, `pub struct Normal { pub x: i16, pub y: i16, pub z: i16 }`, `pub struct Face { pub sides: u8, pub flags: u16, pub color: i16, pub normal: i16, pub vertices: Vec<u16>, pub offset: u16 }`, `pub struct FaceList { pub center: u16, pub offsets: Vec<u16> }`; `pub fn read_object(bytes: &[u8]) -> Result<ObjectData, AssetError>` — `name` keeps the file's quotes as bytes; `flags` is the record's high byte's bits (`& 0xFF00`), `sides` the low byte; the leading `0` word of `POLY` is skipped; each `Face.offset` is its byte offset from the start of the face data, the base the lists index; `lists[0]` is `ORD0`, `lists[1..]` the `ORDE` blocks in file order; the final `0` word of a list is not stored.

**Steps:**

- [ ] **Step 1: Write the reader tests first.** In `crates/sr-assets/tests/visu.rs`, reading `../../assets/VISU/C/SCENE/U2A.00M` (796 bytes, `city=0`, 6 objects `[1,2,3,1,1]`, palette entry values match the file's bytes at 16..784); `U2E.00M` (900, 58 objects, duplicates as listed); `U2A.0AA` (`[1]`); `U2A.0AB` (`522` frames, `1,272` ops, every frame's `fov=0x22` except the final END frame; frame 0 has one camera op at `(-221,-323,7088)` with 5 matrix deltas; frame 16 toggles object 2 on with `dy=-9178`; frames 259/270 toggle 1+4 and 5 on; 514/517/519 off; 520 on); `U2E.0AB` (`1,802` frames, `4,047` ops, `fov=0x1C`); `U2A.001` (name `b"\"s01\""`, 159 vertices, 269 normals, 124 faces, 9 lists, first face sides 8, flags 0x1C00, color 32, vertices `[4,5,6,7,0,1,2,3]`); `U2E.001` (29/1/5); every offset in every shipped list resolves to a face's `offset` (this pins the list base against `VISU.C:70-95`); and every shipped `.00N` file parses with `sides <= 16` and `vnum == vertices.len()`. A malformed stream (a truncated frame, an object index equal to `conum`) returns `Err`, never panics.
- [ ] **Step 2: Run the tests; fail.**
- [ ] **Step 3: Implement the readers** in `crates/sr-assets/src/visu.rs`, following `SAVE.C:40-90`, `VISU.C:25-96`, `MAIN.C:252-321` and the parts' loaders (`U2A.C:148-219`, `U2E.C:208-263`). Bound every length against the file before indexing; the `VERS` value is stored, not enforced (the loader only printed).
- [ ] **Step 4: Wire the decode check into `xtask`.** Extend the asset verification to parse every `assets/VISU/C/SCENE/*.{00M,0AA,0AB,001..042}` through the new readers, and update the pinned counts in `xtask/tests/assets.rs`/`verify.rs` to the measured totals (the current tree reports 158 assets / 95 headerless / 1 refused). `cargo test -p xtask --test assets --test verify` passes.
- [ ] **Step 5: Record the new decode coverage** in `docs/fidelity/asset-inventory.md` if the pinned narrative names the decoder set.
- [ ] **Step 6: Run `cargo test -p sr-assets -p xtask` and commit.**

**Acceptance:** the reader tests pin every shipped file's counts and the two streams' frame/command totals; malformed inputs error; the xtask decode pass covers the VISU formats with updated pinned counts.

---

### Task 3: The fixed-point maths

**Files:**
- Create: `bins/secondreality/src/parts/visu/mod.rs` (module skeleton exposing `math`), `bins/secondreality/src/parts/visu/math.rs`
- Modify: `bins/secondreality/src/parts.rs` (module declaration only)
- Test: unit tests in `math.rs`

**Interfaces:**
- Produces, consumed by Tasks 4–7:
  - `pub const UNIT: i32 = 16384; pub const UNITSHR: u32 = 14;`
  - `#[derive(Clone, Copy, Default)] pub struct Matrix { pub m: [i32; 9], pub x: i32, pub y: i32, pub z: i32 }`
  - `pub fn apply_matrix(dest: &mut Matrix, apply: &Matrix)` — `dest = apply × dest` (the asm's order), then rotate `dest`'s position by `apply` and add `apply`'s position (`ACALC.ASM:388-413`); matrix multiply uses `mulmatrices`' low-32 accumulation and `sar 14` (`:67-143`), position rotation uses `rotatesingle`'s 64-bit `shrd 14` (`:415-468`).
  - `pub fn rotate_vertices(out: &mut [Rotated], verts: &[VertexData], m: &Matrix)` — reads matrix entries as signed 16-bit (`movsx`), 64-bit products, `shrd 14`, adds position, truncates to 32 bits (`ACALC.ASM:512-578`).
  - `pub fn rotate_normals(out: &mut [RotatedNormal], normals: &[NormalData], m: &Matrix, count: usize)` — 16-bit normals, same 64-bit path (`:621-681`).
  - `pub struct Projection { pub mul_x: i32, pub mul_y: i32, pub add_x: i32, pub add_y: i32, pub clip_x: [i32; 2], pub clip_y: [i32; 2], pub clip_z: [i32; 2] }`
  - `pub fn window(x1: i32, x2: i32, y1: i32, y2: i32, z1: i32, z2: i32) -> Projection` — `add_x=(x1+x2)>>1`, `add_y=(y1+y2)>>1`, the two parts' call yielding centre `(159,99)` and clips `0..319`, `25..174`, `512..9_999_999` (`AVID.ASM:92-115`, `U2A.C:247`).
  - `pub fn camera_angle(angle: u16, aspect: i32) -> (i32, i32)` — **the parameter is a full angle; the asm halves it** (`shr bx,1`), clamps the halved value to 512..16383, then `shr bx,5` and clears the low bit for the word table's byte offset, so the index is `clamp(angle>>1, 512, 16383) >> 6` (`AVID.ASM:60-85`); `mul_x = (clip_x[1] - add_x) * avistan[index] / 256`, `mul_y = mul_x * aspect / 256`. The `avistan` table is transcribed verbatim from `VISU/AVISTAN.INC` (256 words, `256/tan`); tests pin `(0x2200, 225) -> (360,316)`, `(0x1C00, 225) -> (446,391)` and the table entries `avistan[68] = 577`, `avistan[56] = 715` against the file. (The survey's 145/127 and 194/170 indexed with the stream angle directly — a misread of the halving; the source and the 47.8°/39.4° stream bytes are the authority.)
  - `pub struct PVert { pub x: i16, pub y: i16, pub vf: u16 }`, `pub const VF_NEAR: u16 = …` and the other flag bits from `AD.INC`/`ACALC.ASM`; `pub fn project(src: &Rotated, out: &mut PVert, p: &Projection)` — clamp `z < clip_z[0]` to `clip_z[0]` and set `VF_NEAR`, flag far, compute `y=(y*mul_y)/z + add_y` then `x=(x*mul_x)/z + add_x` with truncating division, set up/down/left/right from the clips, store 16 bits (`ACALC.ASM:759-817`). `pub fn project_all(rotated: &[Rotated], out: &mut [PVert], p: &Projection) -> u16` returns the AND of all `vf`.
  - `pub fn single_z(verts: &[VertexData], index: u16, m: &Matrix) -> i32` — the transformed z of one *local* vertex (`calc_singlez(center, v0, r)`, `ACALC.ASM:585-612`), used for the object sort.

**Steps:**

- [ ] **Step 1: Write the failing tests.** Hand-computed cases for `mulmatrices`' low-32 path (a product whose low 32 bits differ from the 64-bit quotient after `sar 14`), `rotatesingle`'s 64-bit path, a signed-16 matrix entry (e.g. `0x8000` read as `-32768`, not `32768`), `project`'s truncation (`(-7*145)/3 == -338`, not `-339`), the vf flags at each clip boundary, `single_z`, `camera_angle`'s two pairs (360/316 at `0x2200` and 446/391 at `0x1C00`, aspect 225) with the halving and the 512..16383 clamp, and `window`'s centre.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the maths**, transcribing `ACALC.ASM`'s integer paths exactly. Compute products in `i64` and reproduce each shift (`shrd`/`sar`) at the width the original used; do not "simplify" `mulmatrices` into the 64-bit path.
- [ ] **Step 4: Run the unit tests and commit.**

**Acceptance:** every quirk above has a test that fails on the clean implementation and passes on the transliteration; `camera_angle` returns 360/316 and 446/391, and its 512..16383 clamp is pinned at both ends.

---

### Task 4: The polygon pipeline

**Files:**
- Create: `bins/secondreality/src/parts/visu/draw.rs`
- Test: unit tests in `draw.rs`

**Interfaces:**
- Consumes: Task 3's `PVert`, `Projection`, `Rotated`, `RotatedNormal`, flag bits.
- Produces, consumed by Task 5:
  - `pub struct Page<'a> { pixels: &'a mut [u8], width: usize }` — the 320×200 page being drawn.
  - `pub fn cull(face: &Face, object_flags: u16, rotated_normals: &[RotatedNormal], rotated_vertices: &[Rotated], screen_flags: u16) -> bool` — the `ADRAW.ASM:74-95,484-493` rule: skip `color == -1`, skip when not two-sided and the sign bit of `dot(normal, first_vertex)` is clear; the face's material bits (`flags & 0xFF00` combined with `object_flags & 0x0F00`) select the light path. **`screen_flags` is the AND of the face's (or object's) projected `VF_*` flags, or 0 — never the OR**, which would drop visible clipped faces (`draw_polylist`'s `dh`/`dl`).
  - `pub fn poly_flags(face_flags: u16, object_flags: u16) -> u16` — the `ADRAW.ASM:466-471` combination (`or 0x0F00`, `and` the object's mask); and `pub fn light(normal: &RotatedNormal, flags: u16) -> u8` — `normallight`: `dot(rotated_normal, (12118,10603,3030)) >> 16 >> 5 + 128`, clamped 0..255 (`ADRAW.ASM:20-72`); `calclight`: if no shade bit (`flags & 0x0C00 == 0`) return 1, else shift by `6 - (flags>>10)` and clamp 1..30; the returned value is added to the base colour. The combined flags decide both the light path and whether `gouraud_colors` is `Some` (`F_GOURAUD`).
  - `pub fn clip_and_fill(page: &mut Page, face: &Face, rotated: &[Rotated], pverts: &[PVert], projection: &Projection, gouraud_colors: Option<&[u16]>, color: u8)` — `newclip`'s order: drop if all vertices share `VF_FAR`; near-clip first if any `VF_NEAR` (`ADRAWCLP.ASM:244-296`), interpolating x/y **in 3-D using the rotated vertices** and re-projecting, **copying the previous vertex's Gouraud colour** (`:229-242`, the preserved TODO bug); then up, down, left, right Sutherland–Hodgman passes in projected space; then span generation and fill.
  - The span path: `poly_nrm`/`poly_grd`'s top-vertex walk, the 16.16 X start with the `0x8000` bias and per-row increment, right-exclusive rows (`ADRAW.ASM:150-410`); flat rows write the colour byte; Gouraud rows reproduce `AVIDFILL.ASM`'s 8.8 colour/adder word, the per-2-pixel slope from `65536/len` (`AFILLDIV.INC`), and the three accuracy tiers (1-, 2-, 4-pixel) including the tier selection by slope magnitude.
  - The fill *stream* itself (`_polydrw`, `AVIDFILL.ASM`'s planar masks, `faststosb`) is a VGA-planar implementation detail and is **not** ported: a linear page can be filled directly. The emitted pixel set and colours are what must match, and the tests prove it.

**Steps:**

- [ ] **Step 1: Write the failing tests.** A 3-4-5 triangle and a quad with hand-computed fills, including the right-exclusive right edge and the half-pixel bias; a polygon crossing only the near plane (colour copy bug visible in the Gouraud case); one crossing each window edge; a 12-gon through all five planes (assert a simple polygon, no panic); a two-sided face visible with a positive dot; a `color == -1` face skipped; `light` values for the eight principal normals and a shade-band sweep pinning the 1..30 clamp.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the pipeline**, transliterating `ADRAW.ASM`/`ADRAWCLP.ASM`/`AVIDFILL.ASM`. Keep the original's near-clip colour copy, the near clip's 16-bit signed colour difference, the strict/visible-clipper boundary rules and the exact tier arithmetic; widen the internal vertex cap to the five-pass worst case (16·2⁵ = 512) with a comment naming the original's 16-side overflow.
- [ ] **Step 4: Run and commit.**

**Acceptance:** the synthetic polygon and lighting tests pass; the near-clip Gouraud bug is reproduced; the 12-gon clip does not panic.

---

### Task 5: The scene driver and page ring

**Files:**
- Create: `bins/secondreality/src/parts/visu/mod.rs` (driver; expose `math`, `draw`)
- Test: unit tests in `mod.rs`

**Interfaces:**
- Consumes: Tasks 2–4 (`ObjectData`, `Master`, `Animation`, maths, `draw`).
- Produces, consumed by Tasks 6–7:
  - `pub struct Instance { pub object: Rc<ObjectData>, pub r: Matrix, pub r0: Matrix, pub on: bool, pub index: u16 }`
  - `pub struct Scene { pub master: Master, pub instances: Vec<Instance>, pub animation: Animation, pub cam: Matrix, pub stream: usize, pub xit: bool }`, built by `pub fn load_scene(dir: &Path, name: &str) -> Result<Scene, String>` — reads `<name>.00M`, the object files the table names (duplicates share `Rc<ObjectData>` and get fresh matrices), `<name>.0AA` and its first scene stream; `reset()` zeroes every `r`/`r0`, rewinds the stream.
  - `pub struct Ring { … }` — the four-page state machine: `ready[4]` (0 free, 1 queued, 2 displayed-then-free), `clr`, `clw`, `copperdelay`, `syncframe`, `currframe`, `avgrepeat`, `repeat`, and the per-cadence `frames_per_step` (U2A 1, U2E 2). `pub fn isr(&mut self)` is the `copper2` tick; `pub fn next_page(&mut self) -> Option<usize>` frees the writer's page or returns `None`; `pub fn queue(&mut self, page: usize, frames: usize)`; `pub fn displayed(&self) -> usize`.
  - `pub enum Clear<'a> { Background(&'a [u8]), Byte(u8) }` — `vid_clearbg` (U2A) versus `vid_clear255` (U2E).
  - `pub struct Driver { pub ring: Ring, pub proj: Projection, pub fov: u16, pub first_draw: bool }` with `pub fn draw_frame(&mut self, scene: &mut Scene, page: &mut Page, clear: Clear<'_>)` — the loop body of `U2A.C:284-324`/`U2E.C:394-431`: clear the page, order the enabled instances (skip camera), `apply_matrix(r, cam)`, `single_z(center)`, the city hacks (U2A, dead for the shipped file but transliterated), the U2E name hacks (`name[1]=='_'` → 1e9; `currframe in 1800..2200` and name `s01` → 1), the stable descending insertion sort, then per object `vis_drawobject`: rotate vertices and normals (Gouraud only when the object needs them; the original's TODO), project, pick the face list by the minimum z of its sort vertex (`plnum==1` → list 0), and draw each face. Each face's combined flags come from `poly_flags(face.flags, object_flags)`; `cull` gets the AND of the object's projected `VF_*` flags (or 0); `gouraud_colors` is `Some` exactly when the combined flags carry `F_GOURAUD`.

**Steps:**

- [ ] **Step 1: Write the failing driver tests.** Instance construction from `U2A.00M` (6 instances, duplicates sharing the same object pointer, distinct `r0`); the sort order on a two-object synthetic scene with known z; the list-selection rule on a synthetic two-list object; the ring state machine for 1- and 2-retrace cadences over 20 ticks (page queue/display/free order and `displayed()` per tick); the first-frame flags (U2A draws an empty background, U2E skips).
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the driver**, transliterating `VISU.C:101-125` and the parts' loop bodies. The driver is generic over the background/clear choice (`Bg(image)` for U2A, `Clear255` for U2E) and the cadence.
- [ ] **Step 4: Run and commit.**

**Acceptance:** the instance/sort/list/ring tests pass; the driver renders U2A script frame 0 as the background, draws object 2's pixels on the first on-frame that projects inside the page, and — on the shipped window `(0,319,25,174)` — still culls the saucer at frame 16 because every projected vertex is above `clip_y[0] = 25`; it first enters the window at frame 22. Both are pinned (`U2A.C:247`, `VISU.C:111-112`).

---

### Task 6: U2A — the landing pad and the saucer

**Files:**
- Create: `bins/secondreality/src/parts/u2a.rs`, `bins/secondreality/tests/u2a.rs`, `docs/fidelity/u2a.md`
- Modify: `bins/secondreality/src/parts.rs` (registry row), `bins/secondreality/src/shell.rs` (the expected-music row; the block-1 stop unit test), `docs/fidelity/asset-inventory.md` if a new asset note is needed
- Test: the part's unit tests, `bins/secondreality/tests/u2a.rs`

**Interfaces:**
- Consumes: Task 2's readers, Tasks 3–5's engine, `PartCtx` (`surface`, `view`, `palette`, `set_palette`, `set_copper`, `music`, `exit_requested`), `sr_assets::load_path`.
- Produces: `pub struct U2a`, registered as `"u2a"`; `music_for_part("u2a")` is already MUSIC0@0 and `music_start_frame_for("u2a")` is 400.

**The transliteration, from `VISU/C/U2A.C` (re-derive each line):**
- **Load:** `load_scene("assets/VISU/C/SCENE", "U2A")`; read `U2ABG.UH` with `sr_assets::load_path` (its payload is already linear 320×200; **do not** port the planar de-interleave); merge the palette: entries 0–191 from `U2A.00M`, entries 192–255 from the background's entries 0–63 (`U2A.C:152`).
- **Setup:** set the view to 320×200 at page origin 0 with `y_line_offset = 320`; `vid_init(3)` is *state only* in the original (no VGA mode write); `vid_window(0,319,25,174,512,9999999)`; copper slot 2 registered on the ring's `isr`, the callback setting the view origin to the displayed page ×64,000. **Before gate 1** (the measured release behaviour): paint the U2ABG payload into the displayed page(s) and upload the measured pad palette `(3 × U2ABG palette + 5) & 63`, entry 195 black — the 6500 keyframe is inside this hold. **At gate 1:** upload the merged scene palette and begin the loop.
- **Gate 1:** a `frame()` phase that polls `ctx.music().position()` once per tick for `order > 10 && row > 46`, returns `Continuing`, checks `ctx.exit_requested`; it draws nothing and leaves the surface/palette as inherited.
- **Loop:** each tick: check gate 2 (`order > 11 && row > 54` → teardown); wait for a free page (`ring.next_page`, bounded by the ring's `deadlock` counter); clear the page with the background; camera angle from the current `fov` (the first drawn frame uses `fov = 0`, the uninitialised value's substitution, invisible because every object is off — note it); drive one frame; queue the page; parse one stream frame (apply ops, then set `fov`); when the stream's END arrives, `xit`.
- **Teardown:** remove the copper callback, clear all four pages with the background, leave the view on the last cleared page, return `Finished`. The screen holds until PAM's own wait retires it.
- **Fidelity note:** the measured tick map (wait, pad, animation ≈6990–7510, cleared hold), the gate positions, the palette merge, the dead city hacks, the uninitialised `fov`, the background layout, and the reference comparison — every number by a named command or test.

**Steps:**

- [ ] **Step 1: Write the failing unit tests.** Palette merge (entry 192 equals the background's entry 0, entry 191 equals the scene's); the gate phase advances only on the scripted cue and exits on `exit_requested`; the stream's first frame leaves every object off; frame 16 turns object 2 on; the teardown clears all four pages; the ring's first displayed page is the cleared background.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the part**, transliterating `U2A.C:145-451`. The wait is phases, not loops; `ctx.music()` is polled once per tick.
- [ ] **Step 4: Add the registry and shell rows.** `parts::registry` gains `("u2a", || Box::new(u2a::U2a::new()))`; the expected-music table gains `("u2a", Some((Music0, 0)))`; the block-1 stop unit test's expectation moves from `u2a` to `glenz` (the plan now reaches PAM/BEGLOGO and stops at GLENZ).
- [ ] **Step 5: Write the integration test.** `--block 1 --headless --dump-frames 6500,7299,7300,7301,7640` from the repository root; read the rendered manifest; compare with the length-asserted byte comparison pattern of the other part tests: 0 differing pixels at 6500 (RGB) and 7640 (indices and RGB), and the measured registered divergence at 7300 (1098 boot-index pixels; state 294's own render leaves 426, A28/A29). `cargo xtask compare --reference reference/parts/u2a --rendered <out> --out <diff>` reports 0.0000% at 6500/7640 and the registered 1.7156% at 7300. 6500 is the held pre-gate screen (Task 1's finding); 7300's supports are required because it moves; 7640 is held.
- [ ] **Step 6: Write `docs/fidelity/u2a.md`** with the note sections the other parts use, the measured tick map, the pad resolution, and the reproducible commands.
- [ ] **Step 7: Run the part's unit tests, the integration test, and `cargo test -p secondreality`; commit.**

**Acceptance:** 6500 and 7640 compare at 0.0000%; 7300's reference picture is stream state 294, whose render leaves 426 registered Gouraud pixels (A28: no tree-carriable constant, the flat object and silhouette exact), and the boot-index picture differs by the measured 1098 because the keyframe capture's session music phase differs from the block-1 clock's (the port matches `second1`'s `9:51` at 5900; `second1c`, source of the 6988 gate, reads `9:50`/zframe 3768 — D17's session spread, A29). The unit tests pin the gate, the palette merge, the teardown, the state-294 render and the measured divergence; `--block 1` reaches U2A in the chain (no anchor needed: the run is numbered from ALKU's 404).

---

### Task 7: U2E — the city flythrough

**Files:**
- Create: `bins/secondreality/src/parts/u2e.rs`, `bins/secondreality/tests/u2e.rs`, `docs/fidelity/u2e.md`
- Modify: `bins/secondreality/src/parts.rs` (registry row), `bins/secondreality/src/shell.rs` (the expected-music row; the block-stop unit tests), `docs/fidelity/asset-inventory.md` if needed
- Test: the part's unit tests, `bins/secondreality/tests/u2e.rs`

**Interfaces:**
- Consumes: Task 2's readers, Tasks 3–5's engine, `PartCtx` as above.
- Produces: `pub struct U2e`, registered as `"u2e"`; `music_for_part("u2e")` is already MUSIC0@18 via block 4.

**The transliteration, from `VISU/C/U2E.C` (re-derive each line):**
- **Load:** `load_scene("assets/VISU/C/SCENE", "U2E")`; the city byte is 0, so the city hacks never fire but are transliterated.
- **Jelly check and intro:** model `jellywas` as the original's mode query. In a run where a part has already drawn (block 1/2/3 reaching U2E), the inherited DAC is captured; in a block-4/`--part u2e` run the shell's boot palette is the inherited DAC (record this substitution in the note; no keyframe covers the intro). Then the 100-tick intro: 33 steps of +2 to `fpal[3..762]` with one tick each and an upload; 16 ticks; `fadeset` writes the border/letterbox pixels into all four pages, one tick, CRTC 9 `(old & 0xf0) | 0x80`, one tick, `fadeset` the other three pages; 16 ticks; 33 steps of −2 to `fpal[3..759]` and +2 to `fpal[759..765]` with one tick each. `fadeset`'s exact pixel writes are transliterated from `U2E.C:145-183`. The CRTC write is a display-hardware detail: the port's view stays 320×200, and the note says the reference measures 320×200.
- **Main setup:** `vid_init(11)` is state only; the palette is `U2E.00M` entries with 252 black, 253/254 white (63), 255 black (`U2E.C:337-350`); window as U2A; copper slot 2 on the ring.
- **Gate:** a `frame()` phase polling for `order > 18`, one poll per tick, exit-aware.
- **Loop:** ring init (`cl[3].ready=1`, the first displayed page is the fadeset page); the first loop iteration skips the draw (`firstframe`) and resets `syncframe`/`currframe`; thereafter each iteration draws, queues, and advances `repeat = (a+1)/2` stream frames with `currframe += repeat*2` — one stream frame per two ticks, with the ring displaying each page for the corresponding number of retraces. The `currframe` name hack (`1800 < currframe < 2200`, strict, name `s01` → distance 1) uses the original's units. The stream's END sets `xit`.
- **Teardown:** remove copper, the 16-step DAC fade starting at index 255 and wrapping through 0 (768 writes; the last three entries' green/blue bytes are left as the original leaves them), then `Finished`.
- **Fidelity note:** the tick map against the measured milestones (420 loader black, ~anchor first fade, 500 fade, 600=700 hold, 800 corridor, 1000/1600/2600/3600 keyframes), the gate position from `second4`, the two-boot behaviour, the jelly substitution, and the reference comparison.

**Steps:**

- [ ] **Step 1: Write the failing unit tests.** The intro's 100-tick length and the fade's first/last palette values; the **tick map** — with the run at the anchor, the fadeset is drawn at tick 49 (33 fade-in waits + 16 waits, before the fadeset's own wait), so the first visible frame is anchor+49 = 467; 500 is inside the fade-out and 530 inside the post-fade hold (these two are the measured screens; the note records their hashes and the capture command); `fadeset`'s first pixels; the gate consumes ticks until `order > 18`; the first loop tick draws nothing and consumes one stream frame; the cadence over 8 ticks advances 4 stream frames; the `s01` hack window; the teardown's 768 writes.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the part**, transliterating `U2E.C:185-559`. The waits are phases; no loop spins.
- [ ] **Step 4: Add the registry, the anchor and the shell rows.** `("u2e", …)`; expected music `("u2e", Some((Music0, 18)))`; `boot_anchor("u2e") = Some(<intro start>)` — the measured first visible frame is 467 (the fadeset), so the anchor is the intro start ≈ 418, calibrated by Step 1's tick-map test so the fadeset lands at 467; remove the now-false block-stop expectations (block 3 and block 4 no longer stop at U2E) and the `--block 4` refusal test.
- [ ] **Step 5: Write the integration test.** `--block 4 --headless --dump-frames 999,1000,1001,1599,1600,1601,2599,2600,2601,3599,3600,3601` (all four keyframes move; the supports are D19's). The run's numbering is the reference's from the anchor. Compare RGB (no `.idx` is filed) with `cargo xtask compare`, exit 0.
- [ ] **Step 6: Write `docs/fidelity/u2e.md`** with the measured tick map, the intro and fade evidence, the two-boot note, the jelly substitution, and the commands.
- [ ] **Step 7: Run the tests and commit.**

**Acceptance:** the four keyframes compare at 0.0000% in `--block 4`; the unit tests pin the intro, gate, first-draw skip and cadence; the note's tick map matches the measured milestones.

---

### Task 8: The shell — the U2E boundary restart and the run shapes

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (the restart site and its install path, `Sequence`'s stored music source, the block-plan/stop tests; the U2E anchor constant is Task 7's), `bins/secondreality/tests/shell.rs` (the block-3 chain extension, the block-4 run, the block-1 chain extension), `CLAUDE.md` is Task 9
- Test: shell unit tests, `tests/shell.rs`

**Interfaces:**
- Consumes: `Engine::set_music` (re-arms `music_fresh`), `MusicStart`, `MUSIC_RESTARTS`, `MusicSource`, `RunMusic`.
- Produces:
  - `MUSIC_RESTART_AT: &[(&str, ModuleId, u8)] = &[("u2e", ModuleId::Music0, 18)]` — a part-keyed restart site. Its original guard `whattorun & 8` (`MAIN/U2.ASM:880`) is implied by U2E's presence in any block plan; the doc comment says so.
  - `MusicStart::restart(index: u32) -> MusicStart` — `{ frame: index, waiting: MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME, pre: 0 }` (4 ticks; D34).
  - `Sequence` stores `music: Option<MusicSource>` alongside its plan; `take_next` consults `MUSIC_RESTART_AT` before `start(name)` and, when the row's module/order differs from the current `RunMusic`, installs a fresh player with `play_song(order)`, `MusicStart::restart(self.index)`, and carries `AudioDump`/scratch across. The first part of a plan is unaffected (it goes through `with_run`'s entry install, not `take_next`).
- **Block shapes:** block 1's plan runs ALKU → U2A → PAM → BEGLOGO and stops at GLENZ; block 3's plan runs its whole list (… → JPLOGO → U2E → ENDLOGO → CRED → ENDSCRL) and stops at no part, the restart firing at the U2E boundary; block 4 runs U2E → ENDLOGO → CRED → ENDSCRL from U2E's anchor under MUSIC0@18.

**Steps:**

- [ ] **Step 1: Write the failing shell tests.** `a_restart_installs_the_next_modules_order_at_the_u2e_boundary` — a stub plan `[mntscrl, u2e]` with the real `MusicSource` seam (or a scripted one) asserts the music changes to MUSIC0@18 at the boundary, `RunMusic.start.waiting == 4`, the dump's scratch survives, and the engine's fresh flag is armed for U2E. `u2e_runs_are_numbered_from_their_derived_intro_anchor` — checks the constant Task 7 added: `boot_anchor("u2e")` is `Some(anchor)` and `--block 4`'s first index is the anchor. The block-stop tests update: block 1 stops at `glenz`, block 3/4 stop at `None` (ENDSCRL never ends).
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the restart and the anchor** (D34's waiting value).
- [ ] **Step 4: Extend the real chain tests, bounded.** In `tests/shell.rs`, extend the block-1 chain to the first PAM frames, asserting ALKU → U2A → PAM with the reference numbering and the music positions at U2A's start/gates. In the block-3 chain, extend past 16,884 to just past the U2E boundary, asserting the restart fired (MUSIC0 order 18), the part switched to U2E, the geometry is 320×200, and the player's positions match `second3e` at the mapping-derived indices within its row granularity. Do not run U2E's 3,600-tick animation in the chain test; the part's own integration test owns that.
- [ ] **Step 5: Run `cargo test -p secondreality --test shell` and the shell unit tests; commit.**

**Acceptance:** the restart unit test fails without the install and passes with it; block 1 and block 3 real chains assert the new boundaries and the restart; block 4 starts at the derived intro anchor.

---

### Task 9: Close-out — docs, carried debt, and the phase record

**Files:**
- Modify: `docs/fidelity/APPROXIMATIONS.md` (any accepted divergence: the U2E intro's inherited DAC substitution, the uninitialised `fov` substitution, the linear-fill substitution, anything Task 1's pad finding leaves), `docs/decisions.md` (finalize D32–D34 with the measured evidence), `docs/fidelity/asset-inventory.md`, `docs/fidelity/reference-indices.md` (the U2E anchor finding), `CLAUDE.md` (twenty parts, the block examples, the U2A/U2E ESC note), `PROGRESS.md` (Phase 6 done, the evidence section), `docs/fidelity/{u2a,u2e}.md` (finish any numbers Task 1 produced)
- Modify the Phase 5 parked items: `docs/fidelity/APPROXIMATIONS.md` A27's "640×200" label → "320×400"; the "fourteen captures" prose; the unmarked x-scale sweep; the D19 band-corpus amendment (`decisions.md` and `reference-indices.md`: a fit corpus is a third kind of reference, neither keyframe nor trace)
- Test: `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings` (both the default and `--release` invocations the project uses)

**Steps:**

- [ ] **Step 1: Write the fidelity notes' remaining sections** (any evidence Task 1 produced that the part tasks deferred) and the approximation entries with measured differences.
- [ ] **Step 2: Finalize D32–D34** with the measured values (the first-visible frame and the derived anchor, the gate frames, the restart phase from `second3e`).
- [ ] **Step 3: Apply the Phase 5 carried doc fixes.** A27's label; the "fourteen captures" count; the x-scale sweep's marking; the D19 amendment with its reasoning and where fit corpora live.
- [ ] **Step 4: Update `CLAUDE.md`.** The part count becomes twenty; the run examples show `--block 1` running to GLENZ, `--block 3` through U2E to the scrollers, `--block 4` from U2E; the ESC paragraph names U2A/U2E's exit behaviour (both poll `ctx.exit_requested` in their waits and loops; U2A's gate-2 cut is the original's music cut, not an exit).
- [ ] **Step 5: Update `PROGRESS.md`.** Phase 6's row is **done** with the tag withheld until the final review (the project's practice), the evidence: the three traces, the three/four keyframe comparisons per part, the chain/restart tests, the new decisions and approximation numbers.
- [ ] **Step 6: Run the full suite and both clippy gates; commit.**

**Acceptance:** every note's numbers are reproducible by a named command or a same-commit test; the carried Phase 5 items are closed; `CLAUDE.md` and `PROGRESS.md` match the runnable truth.

---

## Self-review notes (for the controller, not the implementers)

- **Spec coverage:** §4.4's deferred scene format → Task 2; §10's "U2A and U2E verified" → Tasks 6–7; the vector-engine risk → the engine tasks; §7's music restarts → Task 8 (the U2E site; the remaining full-run sites stay Phase 9's, as recorded).
- **Task boundaries:** Tasks 3–5 are the engine's three independently reviewable layers; each produces code the next consumes through a named interface. Tasks 6–7 are the two parts; Task 8 is the shell; Task 1 is measurement; Task 9 is the record.
- **Known coupling:** the expected-music table's length assertion forces the registry and table rows into Tasks 6/7; the `--block 4` numbering needs Task 8's anchor, which is why Task 7's step 5 allows the task-local anchor until Task 8 lands it.
- **The plan's one open measurement:** the U2A pad's palette (Task 1, Step 4) decides a branch in Task 6; both branches are written down, and the expected one is named.
