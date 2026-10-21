# Phase 8 — Vault Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the three unreleased development effects — GRID, HARD and TWIST — so each is playable standalone from a vault menu and verified against frames captured from its own original DOS program, without changing the demo's twenty-part sequence.

**Architecture:** A new module tree `bins/secondreality/src/parts/vault/` holding one module per effect plus shared vault metadata. GRID is a mode-13h line rider over the `EYE4.UH` picture with three C-built palettes and its own generated line blit; TWIST is the planar twister with a two-page CRTC flip; HARD is the `B.EXE` state machine (dots, a 3-D grid, a two-pass glenz cube) over a 128-row opaque compositor and a forked, c-prefixed vector engine. All three run silent through the existing `--part` path (registry rows, no `boot_anchor`, `music_for_part` → `None`), and a window-only `--vault` menu selects among them. Reference captures boot the vendored standalone `DIS/DIS.EXE` and each effect EXE (`GRID/P.EXE`, `HARD/B.EXE`, `TWIST/T.EXE`) under the existing oracle; the tool gains a configurable autoexec boot anchor, and a committed staging script reproduces the captures.

**Tech Stack:** Rust 2021, `crates/sr-engine` (Part API, surface/view/palette, copper/music seams), `crates/sr-assets` (Uh1 and LBM loaders; font strips for the menu), `xtask` (reference capture/compare/verify), SDL2 only in the binary. No new crate and no new dependency.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §1 success criterion 2 (`:31-32`), §4.7 `sr-vault` (`:403-407`), §5.1/§5.2 (bugs and generated loops), §10 Phase 8 (`:705`), §12's vault row. Binding rules: `docs/decisions.md` D2, D3, D7, D8 (amended), D15–D20, D24, D25, D29; `docs/fidelity/APPROXIMATIONS.md`, `docs/fidelity/bugs.md`, `CLAUDE.md`.

## Global Constraints

- **The reference is the authority; the effect EXEs are the artifacts.** GRID, HARD and TWIST never shipped in `SECOND.EXE`. Their artifacts are `GRID/P.EXE`, `HARD/B.EXE` and `TWIST/T.EXE` (built by their MAKEFILEs and linked with `..\dis\disc.obj`), run under the vendored standalone `DIS/DIS.EXE`. `DOLINE.EXE`, `DOLOOP.EXE`, `DOL.EXE` and `HARD/V.EXE` are generators/tools, not effects. D24's procedure applies: measure what the artifact does from captured frames before deciding; adopt a release value only when it is one expression or constant the tree's algorithm can carry and every other difference is accounted for; otherwise keep the tree's translation and register the divergence with a measured difference and a lead.
- **Port the algorithm, not a resemblance.** The effects are compiled `cl /AL`: every C `int` is 16 bits. Signed and unsigned wraparound, uninitialised locals, 32-bit `long` geometry, 64-bit asm products (`shld >>15`), truncating division and the source's bugs are transliterated (D7). Generated/unrolled code (`GRID/LINEBLIT.INC`, `TWIST/TWSTLOOP.INC`, `HARD/LINEBLIT.INC`) is replaced by an equivalent loop; a test proves the substitution over its domain against the upstream generator/table, and any difference is recorded.
- **Assets are read in their original formats.** GRID reads `assets/GLENZ/EYE4.UH` (byte-identical to `GRID/EYE4.UH`; `assets/PROVENANCE.md:226`, `docs/fidelity/asset-inventory.md:185`). HARD's four dot streams come from `FCRZ.U` through `BDO.C`'s extraction; HARD's 400-byte grid table is recovered from `B.EXE`/`_GRID1.TMP` with the one-byte question measured. TWIST needs no runtime file. `reference/` and `upstream/` are input-only; the sanctioned writes are music traces and capture output under `reference/parts/`; `out/` is scratch.
- **6-bit palette internally**, widened only at upload. The port replicates the source's palette construction byte for byte, including HARD's 96-byte `loadpal(pal,32*3)` upload in part 1 against 768 elsewhere, GRID's three palettes and their `x&1/x&2/x&4` additions, and TWIST's 768-entry static palette.
- **Never spin-wait.** One `dis_waitb`/VGA-retrace poll is one engine tick; all three effects are tick machines. ESC (`ctx.exit_requested`) ends a part where the source consults `dis_exit`/`kbhit`; TWIST keeps its 4,444-frame cap. No vault part has a timed teardown, so `drains_after_exit` stays false.
- **Standalone DIS semantics, not INDEMO `zinfo`.** Vault runs are silent: `music_for_part` returns `None`, no player is installed, and `ctx.music()` is `None`. The vendored non-INDEMO DIS returns 0 from `dis_muscode` until a key, then the requested code once. The port maps the single start key to the part's start: HARD's `B.C:138` gate passes at start and `BP.C:263`'s `0xff` cut is unreachable without a second key; GRID's `getch()` is a start event and its `dis_muscode(3)` never fires; TWIST's `kbhit()` is the exit flag. Each substitution is registered with the reference behaviour it reproduces (the captures use one key for GRID/HARD and none for TWIST).
- **`sr-engine` and `sr-audio` keep zero runtime dependencies; `sr-assets` may depend on `sr-engine` only.** The vault parts are binary modules under `parts/vault/`; no new crate.
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29).
- **Tests and clippy are the gate**: `cargo test --workspace` green, zero warnings, both clippy commands clean. Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

Five failure modes the spec implies but no single task's tests would otherwise catch. Each is pinned by a named test in the task that owns the code.

1. **The C is 16-bit, and its bugs are the picture.** HARD's `part2init` reads an uninitialised `b` for entries 1000..3099 (`BP.C:130`); `do3dots`'s signed `jg` skips positive z > 3000, so the positive `z>3099` read is unreachable, the negative-low-word branch is unreachable in part 2 (the projected z′ is full ≈1134..4366, drawn ≤3000, always with a positive low word) and its zero is a registered model (A51), and its `bp` is a 16-bit load of a dword z (`BA.ASM:232-235`); GRID's `sinus1` loop uses `d+=67` before `d` is initialised; TWIST's `r` is a wrapping 16-bit `int` (the first in-frame wrap is at frame 327) and `w` truncates toward zero. A clean 32-bit rewrite changes pixels. The tests pin the source's exact values and overflows.
2. **The start/cue substitutions must neither hang nor cut.** HARD's `while(dis_muscode(1)!=1 && !dis_exit())` waits forever against the port's `muscode` 0, and its `dis_muscode(0xff)` cuts part 3 only after a second key; GRID's loop breaks on `dis_muscode(3)==3` if a key reaches the DIS. The vault has exactly one start event and no later key. Tests pin HARD's full mode sequence and the unreachable cut, GRID's continuing loop, and TWIST's 4,444-frame cap.
3. **Display mapping is part of the effect.** HARD's work buffer at `A2D0:0000` is visible only at screen rows 36..163 inside a letterbox, and part 3 never clears it; TWIST flips two pages through the CRTC start register with chain-4 off; GRID writes a linear 64 KB mode-13h screen. The port's view/origin/stride must produce the captured 320×200 image, not a plausible one. Reference comparisons at the measured `K0` pin it.
4. **The generated loops' fixed point and tables are the effect.** GRID's line blit, TWIST's `twstloop`, HARD's dot and grid tables: a loop substitution is valid only if it matches the generated code/data over its domain. Tests read the upstream generator or table and compare the full domain.
5. **Non-fidelity integration must not disturb existing runs.** New registry rows break the music-coverage test (`shell.rs:2126-2205`); vault names must stay out of `PART_ORDER`, `BLOCKS` and `boot_anchor`; `--vault` is window-only and rejects headless/`--part`/`--block`; a silent run refuses `--audio-dump`; the menu must not change ESC semantics for normal runs.

---

## The measured ground truth

Every row is from the carried Phase 1–7 surveys and this phase's capture study; **the implementer re-derives each against the file and the capture**, because the plan is an argument and the artifacts are the evidence.

| Item | Source | What it does | Music/input | Assets | Reference |
|---|---|---|---|---|---|
| GRID effect | `GRID/MAIN.C` 184, `ASM.ASM`, `LINEBLIT.ASM/.INC`, `P.EXE` 71,170 B | `dis_partstart`; mode 13h; `sin1/sinus1/sinus2` with the measured **`d0 = 0x07FE`** (the C's `sin256`/`muldata` are dead); the shift `u<64684` leaves the **last 100 bytes unshifted**; `getch()`; main loop rebuilds 1,001 `xyzdata` records, `asmloop1()`, border 0/1, `dis_waitb`, `setpalette(pal[plane])`; the blitter **excludes the endpoint** and uses **`lblt_54b56` (the (−10,−8) line)** outside its 357-entry table; exits on ESC or `muscode(3)==3` | one start key (`getch`); `muscode(3)` never fires without a later key | `EYE4.UH` (Uh1, identical to `assets/GLENZ/EYE4.UH`) | route A last static 351, **first changed 352 (transition; 353 is the first reproducible loop frame)**; route B 702/703/704; frame 900 matches **8,507/8,507** px under the full model, frames 800/1000 at 8,162/8,164 and 9,353/9,353 (mid-draw top-row pixels); the captured frame carries the previous iteration's palette |
| TWIST effect | `TWIST/MAIN.C` 110, `TWIST.ASM`, `ASM.ASM`, `TWSTLOOP.INC` 11,752 lines, `T.EXE` 39,674 B | `dis_partstart`; `inittwk` (chain-4 off, CRTC R20/R23, clear 64K); 256-entry palette with the DAC's 8-bit→6-bit mask; `doit()` until key or **4,444** frames: `ra++`, `r` a wrapping 16-bit `i16`, `w=trunc(sin1024[((r>>6)&511)+256]*25/64)+100`, row `w%200`; the shipped 200×80-byte texture expanded four pixels per byte; two pages at pixel origins 0 and 131,072, CRTC 0x0c=0x80/0x00 | no DIS key gate; any key exits (`kbhit`); no music | none at runtime (the texture is the shipped `TWSTLOOP.INC`) | route B text at 350, graphics from **355**; 600/800 reproducible; frame 0's two-band content (rows 0..63 row 0, rows 64..199 row 199) is the `K0` fingerprint |
| HARD effect | `HARD/B.C` 393 + `BP.C` (included), `BA.ASM`, `VEC.ASM`/`VNEW.ASM`/`VMATH.ASM`/`VMATHSIN.ASM`/`VDATA.ASM`, `B.EXE` 71,174 B | `dis_partstart`; mode 13h; `muscode(1)` gate; white page, wipe, 70 waits, palette/letterbox; modes 0..6 = 34 + 4×73 + 4 + 42 + 102 + 500 + 800 frames; dots at A2D0 visible rows 36..163; part2 3-D grid; part3 two-pass glenz cube; exit on ESC or `muscode(0xff)==0xff` | one start key passes the gate; the `0xff` cut needs a second key | `FCRZ.U` → four dot streams; `_GRID1.TMP`/`B.EXE` 400-byte grid table | route B last static 706, first changed **707**; 900/1000 reproducible |
| Standalone DIS | `DIS/DIS.EXE` 1,282 B; `DISINT.ASM:112-157,242-282` | non-INDEMO: `muscode` returns 0 until `checkkeys` sees a key, then the requested code once; Esc/Ctrl set `exitflag`; no copper/music | one key is the whole input vocabulary | — | captures run `DIS.EXE` then the effect; the key is scheduled on the emulated clock |
| The capture route | `xtask/src/reference.rs:214-239`, `oracle/dosbox.rs:811-818` | autoexec lines `MOUNT C -RO`, `C:`, optional `AUTOTYPE -w 5 <keys>`, program; `boot()` always waits for `"Soundcard"` | — | — | route B (ECHO/PAUSE batch) proven byte-reproducible; route A (configurable boot anchor) is the planned extension |
| Vault integration | `parts.rs:43-99`, `shell.rs:693-784,1013-1129` | registry rows; unanchored runs number from 0; `music_for_part` fallback `None`; silent runs exist (`testcard`) | silent | — | `--part grid|hard|twist` needs no new shell code |

---

### Task 1: The vault capture route

**Files:**
- Modify: `xtask/src/reference.rs` (`BootWait`, `CaptureOptions`, `validate_options`, `CaptureManifest`, the capture unit tests), `xtask/src/oracle/mod.rs` (a `set_boot_wait` trait method), `xtask/src/oracle/dosbox.rs` (`boot()`'s autoexec anchor), `xtask/src/main.rs` (`--boot-wait` on `reference capture`)
- Create: `tools/vault_capture.sh`
- Test: `cargo test -p xtask --lib reference` and `cargo test -p xtask --test reference` (whichever holds the capture tests); the script's smoke run

**Interfaces:**
- Consumes: the existing capture pipeline (`MOUNT C -RO`, `C:`, `AUTOTYPE -w 5 <keys>`, program) and `DosboxOracle`.
- Produces, consumed by Task 2 and every vault test:
  - `pub enum BootWait { Setup, Graphics }` — `setup` is today's `wait_for_text("Soundcard")`; `graphics` waits for the first graphics frame. Serde name for the manifest is the lowercase word; `#[serde(default)]` is `Setup`.
  - `--boot-wait <setup|graphics>` on `cargo xtask reference capture`, default `setup`, refused unless `--boot autoexec`.
  - `Oracle::set_boot_wait(&mut self, wait: BootWait) -> Result<(), OracleError>` with a no-op default; `DosboxOracle` stores it and `boot()` uses it in the autoexec branch.
  - `CaptureManifest.boot_wait: BootWait`, serde-defaulted `setup`, so every existing manifest still verifies.
  - `tools/vault_capture.sh <grid|hard|twist> --frames A,B,... [--out DIR] [--stage DIR] [--sheet PATH]`: stages `DIS.EXE`, the effect EXE and `RUN.BAT` from `upstream/` into `out/vault-stage/<part>` after checking the four SHA-256s, then runs the capture with `--boot autoexec --boot-wait graphics --cycles 30000`, `--keys "KBD_space"` for GRID and HARD and no keys for TWIST, and `--out reference/parts/<part>` by default. It writes nothing into `upstream/` or `reference/` except the capture output.

**Steps:**

- [ ] **Step 1: Write the failing boot-wait tests.** In `xtask/src/reference.rs`'s test module, pin: `BootWait::default() == Setup`; serde round-trips `"setup"|"graphics"`; a manifest written before the field existed parses with `Setup`; `validate_options` refuses `--boot-wait graphics` with `BootRecipe::Typed`; and the CLI parser accepts `--boot-wait graphics` with `--boot autoexec`. Name them `boot_wait_defaults_to_the_demo_setup_screen`, `a_boot_wait_round_trips_through_the_manifest`, `a_boot_wait_without_an_autoexec_boot_is_refused`, `the_capture_parses_a_vault_boot_wait`.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement `BootWait` and the CLI.** Add the enum beside `BootRecipe`; add the `--boot-wait` arg; thread it through `CaptureOptions`, `validate_options`, `capture()` (call `oracle.set_boot_wait` before `boot`), the manifest, and `DosboxOracle::boot`'s autoexec branch (`Setup` → the existing `wait_for_text`; `Graphics` → `wait_for_graphics(SHELL_TIMEOUT)`). The typed branch is unchanged.
- [ ] **Step 4: Write `tools/vault_capture.sh`.** It stages with `cp` from `upstream/SecondReality/{DIS,GRID,HARD,TWIST}` into `out/vault-stage/<part>` (a fresh empty directory per run), checks `sha256sum` against the four constants below, writes the part's `RUN.BAT` (`@ECHO OFF`, `DIS.EXE`, the effect line), and invokes `cargo xtask reference capture`. The hashes:
  - `GRID/P.EXE` `d4e7f1d6e1b5e5c3ba3fd85fe8f3645c1c99be1f0d200c791968c656b5334ff0`
  - `HARD/B.EXE` `1f8a7bdae29a34af42c3ffde63163661ca82a6884a01cb6eb775e81ac600c81d`
  - `TWIST/T.EXE` `88d5e695a485b87b33bce8646b67f843d410e0fa822e76d8fbbb76cc9d33aab8`
  - `DIS/DIS.EXE` `270f5fe2c7c7d17b4720c2019c2832f4bd3863f97e750315974692911f0ead08`
- [ ] **Step 5: Prove route A.** Run `tools/vault_capture.sh twist --frames 355,500,600 --out out/vault-smoke/twist` and `tools/vault_capture.sh grid --frames 400,500,600 --out out/vault-smoke/grid`. Both must exit 0; the manifests must say `boot: "autoexec"`, `boot_wait: "graphics"`, `exe: "RUN.BAT"`, `keys: []` (TWIST) / `["KBD_space"]` (GRID), and 320×200; the GRID sheet must show the line effect. The measured route-A start is **last static 351 / first changed 352** for GRID, 351 frames earlier than route B; TWIST starts immediately (its first graphics are already present at frame 20). A TWIST index set with a 45-frame gap (`355,400,600`) is refused by D19's spacing after an indexed read of the planar screen; space TWIST indices ~100 apart and record the refusal (reference-indices §24.3). If the 5-second key lands before the effect starts, record the measured first changed frame and use route B's PAUSE batch as the fallback, recording the ruling in the ledger and `docs/decisions.md`.
- [ ] **Step 6: Run the suite and commit.** `cargo test -p xtask`; commit the tool change, the script and the smoke captures' measurements (not the smoke output).

**Acceptance:** `--boot-wait graphics` is parsed, validated, stored, and recorded in the manifest; an existing manifest without the key still verifies; the script reproduces a GRID and a TWIST capture from `upstream/`; no file under `upstream/` or `reference/` is modified except the capture output.

---

### Task 2: The three references and their anchors

**Files:**
- Create: `reference/parts/grid/`, `reference/parts/hard/`, `reference/parts/twist/` (frames + `manifest.json`), `docs/fidelity/reference-indices.md` §24 (the vault capture section)
- Modify: `docs/fidelity/grid.md`, `docs/fidelity/hard.md`, `docs/fidelity/twist.md` (the capture section of each; the rest is written by its part's task)
- Test: `cargo xtask reference verify --tree reference`; `cargo test -p xtask --test verify`

**Interfaces:**
- Consumes: Task 1's `tools/vault_capture.sh` and `--boot-wait graphics`.
- Produces, consumed by Tasks 3–6:
  - Five or more keyframes per effect under `reference/parts/<part>/`, each with `.rgb` and, where the oracle can reproduce the palette, `.idx`; a version-1 manifest per directory.
  - The measured `K0` per part: the reference index of the port's frame 0, measured by matching pictures, not guessed from the key. The port's frame 0 is the first frame the effect draws after its start event.
  - The measured mode/palette boundaries needed for keyframe choice (GRID's `plane` cycle, TWIST's page phase, HARD's mode starts).

**Steps:**

- [ ] **Step 1: GRID sweep and keyframes.** Route A's start is measured at last static 351 / first changed **352**, but **352 is a transition frame** (route B's 703 is the same kind: it differs from every later frame and the two sessions agree only from the next frame). The first byte-reproducible loop frame is **353** (route A) / **704** (route B). Re-run a dense sweep 340..365 in one capture to confirm this, then measure `K0` as the reference index of the **port's frame 0** by matching the port's first composed frame to the capture; start from 353, not from the first-changed 352, and record the exact mapping. Then capture at least five spread keyframes across the palette cycle (e.g. 400, 500, 600, 700, 800) into `reference/parts/grid`. Repeat one keyframe in a second session and confirm it is byte-identical; if it moves by one frame, that is D17/D18's alignment and the comparison's ±1, not a new anchor.
- [ ] **Step 2: TWIST sweep and keyframes.** Route A starts the effect immediately; the boot-wait graphics anchor returns at the first graphics frame (the smoke's probes at 20/40/80 are already animated). Identify effect frame 0 by the two-band fingerprint (rows 0..63 = generated row 0, rows 64..199 = generated row 199 at frame 0; frame f's boundary is `ceil(64/(f+1))` for the early frames) and record `K0`. Space keyframes ~100 apart (e.g. 100, 200, 300, 400, 500 — adjust to `K0` and keep clear of the 4,444-frame cap); do **not** use the brief's 45-frame-gap `355,400,600`, which D19 refuses. Capture at least five into `reference/parts/twist`. Repeat one; confirm.
- [ ] **Step 3: HARD sweep and keyframes.** Capture the start sweep and record the last static/first changed. Use the source's frame counts (`B.C:203-322`: 34 + 4×73 + 4 + 42 + 102 + 500 + 800 after the pre-roll) and picture content to find one keyframe in each mode: mode 0 dots, each of the four part-1 logo phases, mode 3, mode 4's band, part 2's grid, and part 3's cube — at least eight keyframes. Capture them into `reference/parts/hard`; repeat the two most different and confirm. The `0xff` cut is not reachable with one key, so part 3 must be captured at its own `count>=800` end.
- [ ] **Step 4: Measure `K0` for each part.** The port's frame 0 is the first frame the effect draws after the start event. Match a port-independent picture feature: GRID's first changed frame is the first loop frame; HARD's first changed frame is the pre-roll wipe, whose first frame is the port's tick 0 only if the port counts the wipe as a tick (it does); TWIST's first graphics frame after `doit()` begins. Record `K0` with the picture evidence and the ±1 tolerance; do not fit it to a later keyframe.
- [ ] **Step 5: Write the record.** In `docs/fidelity/reference-indices.md` §24: the boot (DIS.EXE + effect + one key or none), the four hashes, the staging script, the manifest fields, the measured start indices, `K0` per part and the keyframe lists. In each `docs/fidelity/<part>.md`'s capture section: the same for that part, plus the command that reproduces it.
- [ ] **Step 6: Verify and commit.** `cargo xtask reference verify --tree reference`; `cargo test -p xtask --test verify`; commit the references and the record.

**Acceptance:** three capture directories with version-1 manifests and per-frame digests; `reference verify` clean; each part has at least five keyframes and a measured `K0`; the staged binaries and batch contents are recorded with hashes and a reproducing command.

---

### Task 3: GRID — the line rider

**Files:**
- Create: `bins/secondreality/src/parts/vault/mod.rs` (the module tree and the vault's name/label table), `bins/secondreality/src/parts/vault/grid.rs` (tables, line blit, palettes, Part)
- Modify: `bins/secondreality/src/parts.rs` (`pub mod vault;` and the `("grid", || Box::new(vault::grid::Grid::new()))` row), `bins/secondreality/src/shell.rs` (the music-coverage table gains `("grid", None)`), `docs/fidelity/grid.md`
- Test: unit tests in `grid.rs`; `bins/secondreality/tests/vault.rs` (new)

**Interfaces:**
- Consumes: Task 2's `reference/parts/grid/` and its measured `K0`; `assets/GLENZ/EYE4.UH` through `sr_assets::load_path`; `sr_engine::part::{Part, PartCtx, PartError, PartStatus}`.
- Produces:
  - `pub const GRID_NAME: &str = "grid"`; `pub struct Grid { ... }` with `Grid::new()` and `impl Part`.
  - `pub fn sin1() -> [i16; 1024]`, `pub fn sinus1() -> [i16; 1024]`, `pub fn sinus2() -> [i16; 1024]` — the C's live tables, with the source's exact arithmetic. `sin256` and the 64 KB `muldata` are computed by `MAIN.C` but no linked asm reads them (`grep` over `ASM.ASM`/`LINEBLIT.INC` finds no `muldata`); they are dead and not ported, recorded in the note.
  - `pub struct Eye { pub palette: [u8; 768], pub pixels: [u8; 64000] }` and `pub fn load_eye(path: &Path) -> Result<Eye, String>` applying the source's **buggy shift bound**: image bytes 0..63,899 are `(v << 3) & 0xff`, and bytes 63,900..63,999 (row 199, x=220..319) are left unshifted (`MAIN.C:141`'s `u < 64684` stops 100 bytes short of 64,784). The unshifted tail is required behaviour: it renders the eye palette's cyan group under the line tints.
  - `pub fn palettes(eye_palette: &[u8; 768]) -> [[u8; 768]; 3]` — base `eye[16+(x>>3)*3+{0,1,2}]/4` (entries 0..31 only), red additions by x's low bits: palette 0 `bit0+40, bit2+20, bit1+10`, palette 1 `bit1+40, bit0+20, bit2+10`, palette 2 `bit2+40, bit1+20, bit0+10`, clamp 63; green and blue never exceed 15.
  - `pub struct LineBlitter { vram: [u8; 65536], buf: [u8; 65536] }` with `pub fn line(&mut self, x0: i16, y0: i16, x1: i16, y1: i16, colour: u8)` — the generated `LINEBLIT.INC`'s XOR into both buffers. The generated Bresenham **excludes the endpoint** (the next segment's start is this endpoint, so the polyline stays contiguous without a double draw) and selects its routine by the **low byte** of `(y1−y0, x1−x0)`, not by a signed range. The filled table is dx ∈ −10..=10, dy ∈ −8..=8; every other low-byte entry is the fill word `lblt_table[0]` = the **(−10,−8) line routine** (`ASM.ASM:213-217`, `LINEBLIT.INC:2-4`), not a dot and not nothing. The address is a 16-bit `di + x + y*320` with segment wrap. Cross-line overlap is the union of the three self-XOR line masks, so drawing each segment separately (never XORing a whole polyline mask) is required.
  - The vault table used by the menu: `pub const VAULT: [(&str, &str); 3] = [("grid", "GRID"), ("hard", "HARD"), ("twist", "TWIST")];` (Task 8 consumes it).
- The source's per-frame body (`GRID/MAIN.C:158-177`, `GRID/ASM.ASM:136-202`): `plane = (plane+1) % 3` starting from 0, so the first frame (j=0) uses plane 1 and colour 2, then plane 2/colour 4, then plane 0/colour 1; erase the line drawn in the same plane three frames earlier (replay its stored start and routine with the same colour, XORing it off), rebuild the 1,001 points from `sinus1[(j+i)&1023]+160` and `sinus2[i]+100` for `i=0..=1000` (`d` is set to 0 unconditionally at `MAIN.C:162-163`; the y waveform is static), draw each of the 1,000 segments to the next point and store it; `setborder(0)`, tick, `setborder(1)`, `set_palette(pal[plane])`. The capture's frame drawn in iteration j is displayed under the palette uploaded at the end of iteration j−1, i.e. `pal[j mod 3]` (`GRID/MAIN.C:404`; measured at frames 800/900/1000 = j 97/197/297, planes 1/2/0).
- The display is a linear 64,000-byte mode-13h screen at A000: `vram`/`buf` are 65,536 bytes because the generated blit's `di` is 16-bit and wraps. `init` copies the shifted eye into both buffers and sets the initial palette to the mode-13h default; **the source's 1,024-dot developer screen before `getch()` is not part of the animation** (the eye copy overwrites it at `MAIN.C:156`) and is not ported; it is the measurement that pins `d0`. View 320×200, origin 0, `y_line_offset=Some(320)`, no split. The border is the source's `setborder(0)` during the wait; set `view.border = Some(0)` once (dumps ignore it).

**Steps:**

- [ ] **Step 1: Write the failing sine tests.** Pin the source's expressions (`MAIN.C:38-62`): `sin1[a] = (int)(sin(a*2π/1024)*256)` (expression order `((a*pi)*2)/1024`, truncation toward zero); `sinus1[a] = (sin1[(b>>3)&1023] + 2*sin1[(c>>3)&1023] + sin1[(d>>3)&1023])/8` with `b+=97`, `c+=87`, `d+=67`, 16-bit wrapping adds, unsigned `>>3 &1023`, and signed division truncating toward zero; `sinus2[a] = sinus1[(a+277)&1023]*7/10` likewise. **`d0 = 0x07FE`** for the shipped P.EXE: the research report's exhaustive match over 0..65535 against the capture's 982-pixel static dot screen found only the class `0x7FE mod 0x2000` matching 982/982, while the object's static prediction 0x116 matches 47. The test pins `sinus1()`'s first values (`37,44,50,57,63,68,74,79,84,88,92,96`), the ranges (`sinus1 ∈ [-123,120]`, `sinus2 ∈ [-86,84]`), the `sinus1[1023] = -45 → sinus1[0] = 37` 82-jump seam, and the derived 982-pixel dot set with bbox (37,14)-(280,184). Names: `the_sine_tables_are_the_sources_own`, `the_initial_d_is_the_measured_0x7fe`, `the_sinus_tables_use_the_sources_wrapping_arithmetic`.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the tables.** If the host's `f64::sin` disagrees with the reference's table at any entry, transcribe that entry from the measured source and record the count in the note (D25: the table is a measurement, not a fit; d0 is derived by exhaustive match, not fitted).
- [ ] **Step 4: Write the failing eye/palette tests.** The shift is `for(u=784;u<64684;u++) eye[u]<<=3` over the image bytes at 784..64,783, so the first **63,900** bytes are `(v<<3)&0xff` and the last **100** (row 199, x=220..319) are unshifted; both `vram` and `bufdata` start as the same shifted picture (`MAIN.C:141-142`, `:156`), with no dot screen. The three palettes (`MAIN.C:95-126`) are built from `eye[16+(x>>3)*3+{0,1,2}]/4` (6-bit, entries 0..31 only), with the red additions and clamp as the interface says; assert entries with `x&7==0` are identical in all three, the per-palette ranks, a clamped overlap, and that green/blue never exceed 15. Names: `the_eye_shift_stops_100_bytes_short`, `the_buffers_start_as_the_same_shifted_eye`, `the_three_palettes_are_the_sources_own`.
- [ ] **Step 5: Implement the loader and the palettes.**
- [ ] **Step 6: Write the failing line-blit tests.** The generated routines (`GRID/LINEBLIT.INC`, regenerated byte-identically from `DOLINE.C:13-59` by the research report) are a Bresenham that XORs the colour into `vram` and `bufdata`; it **excludes the endpoint**, selects by the low byte of the deltas, and uses the fill word `lblt_54b56` outside the 357-entry table. For every `(dx,dy)` in −10..=10 × −8..=8 and representative start points, the port's `line` must leave exactly the bytes the direct `DOLINE.C` algorithm XORs; include the endpoint exclusion, a line that crosses the bottom row, a negative direction, a 16-bit offset wrap, and the two measured out-of-domain cases (segment i=746 with dy=56 every frame; the seam segment with dx=82 for j≥24) drawing the (−10,−8) default. A crossing of two different-coloured lines must leave a non-zero tint (the union, not a mask XOR). Name it `the_line_blit_is_the_generated_xor_bresenham`.
- [ ] **Step 7: Implement the line blit.**
- [ ] **Step 8: Write the failing Part/process tests.** `tests/vault.rs`:
  - `a_part_run_of_grid_numbers_itself_from_zero_and_is_silent`: `--headless --part grid --dump-frames 0,1,10`; manifest `index_origin == "part"`, `anchor == null`, `music == null`, frames 320×200 and `part == "grid"`.
  - `grid_matches_the_reference_at_its_measured_start`: for each filed GRID keyframe `R`, dump port frames `R-K0-1, R-K0, R-K0+1` and compare each `.rgb` (and `.idx` where the reference has one) with `reference/parts/grid/frame_{R:06}`; assert the minimum differing-pixel count equals the measured residual (0 if the port is exact). The test prints the three counts and the minimum's offset.
  Name them exactly as above.
- [ ] **Step 9: Implement the Part and register it.** `init` loads the asset, applies the shift bound, copies the picture into `vram`/`bufdata`, builds the tables, sets the view and the BIOS mode-13h default palette; `frame` runs the source's body and returns `Finished` when `ctx.exit_requested` (checked at the top, as `while(!dis_exit())`). The `dis_muscode(3)` break is not reachable with no music and no later key; the note records that (the port's `muscode` is 0, D8). The source's `getch()` developer pause is not ported as a key wait (the port's contract: nothing waits on a key) and its dot screen is not part of the animation; the note records both substitutions and the reference capture's one-key start. The palette lag is matched to the capture: iteration j's content is displayed under `pal[j mod 3]`; if the engine's compose-after-frame convention puts the upload one frame early, set the palette at the start of the next tick or keep the one-frame-delayed mirror, and record the choice.
- [ ] **Step 10: Run the comparison and fix the phase.** Adjust the palette/tick pairing only to the measured reference phase (D17/D18; never re-anchor a constant to make a diff pass). Register every non-zero residual in `APPROXIMATIONS.md` with its measured count and the candidate cause; write `docs/fidelity/grid.md` with the capture, the substitutions, the residuals and the commands.
- [ ] **Step 11: Commit** the part, the tests, the note and the register entries.

**Acceptance:** the GRID tables and line blit are pinned against the source and the generator; `--part grid` runs silent and draws 320×200; every filed GRID keyframe is compared at `K0` with its residual recorded (0 or registered); `cargo test -p secondreality --test vault` passes.

---

### Task 4: TWIST — the planar twister

**Files:**
- Create: `bins/secondreality/src/parts/vault/twist.rs`
- Modify: `bins/secondreality/src/parts/vault/mod.rs` (module), `bins/secondreality/src/parts.rs` (registry row `("twist", ...)`), `bins/secondreality/src/shell.rs` (music-coverage row `("twist", None)`), `docs/fidelity/twist.md`, `bins/secondreality/tests/vault.rs`
- Test: unit tests in `twist.rs`; the process test in `tests/vault.rs`

**Interfaces:**
- Consumes: Task 2's `reference/parts/twist/` and `K0`; `glenz::tables::SIN1024`; `sr_engine::part`.
- Produces: `pub struct Twist` with `Twist::new()` and `impl Part`; `pub fn rows(frame: u32) -> [u8; 200]` (the selected texture row per scanline: `ra = (frame+1) as u16`, `r = (y as u16).wrapping_mul(ra)`, `w = trunc_div(sin1024[((r>>6)&511)+256] as i32 * 25, 64) + 100`, row = `w % 200` with 200 → 0); `pub const LAST_FRAME: u32 = 4444`; `pub const TEXTURE: [[u8; 80]; 200]` — the shipped `TWSTLOOP.INC`'s 200 source rows, 80 plane bytes each, extracted from its `mov ds:[di+N],ax` immediates (left routine bytes 0..39, right routine 40..79).
- **The texture is consumed as shipped, not re-derived.** `TWSTLOOP.INC` is exactly what `DOL.C` produces from `TMP.UH` after its palette-red decode, edge cleanup, `from=6..198` right-half shear and the `v[0]=63` quirk (121 of 200 rows begin `0x3F`). The port transcribes the 16,000 bytes into `TEXTURE` and a test reads `upstream/SecondReality/TWIST/TWSTLOOP.INC` to reconstruct them: 8,000 stores, offsets 0..39/40..79 per row, values 0..63, 121 rows with byte 0 `0x3F`.
- **The visible image** is each texture byte expanded to four identical pixels (the map-mask-15 write replicates the byte into all four planes): for the selected source row `w`, the 80 bytes fill `y*320 + 0..319` in groups of four. `twister()`'s row remap is `w % 200` (not a clamp to 199); `w` is in 1..200 and the sine peak `w=200` selects row 0.
- **The two pages are the source's double buffer.** The surface is 640×400; page 1 is pixels `0..63,999`, page 2 pixels `131,072..195,071` (`A000:0` / `A800:0` with one plane byte = four pixels). Even part frames draw page 1 and set `view.start_addr = 0`; odd frames draw page 2 and set `view.start_addr = 131,072`; `y_line_offset = Some(320)`, width 320, height 200, no split. The page just drawn is the one displayed.
- **The palette uses the DAC's 8-bit-to-6-bit mask.** The C writes `palette[a] = (a/3, 2a/3, a)` as bytes; the DAC masks each to its low six bits. The port must build `((a/3)&63, (2a/3)&63, a&63)` with `Palette::set_entry`, **not** `Palette::from_vga`'s `>>2`; entry 63 is `(21,42,63)`, entry 191 white, entry 192 black. `vram[a]=a` is dead before the first twister draw.
- The loop is 4,444 iterations (`while(!kbhit() && frame<4444)`): frames 0..4,443 draw; the 4,445th check returns `Finished` without drawing; `ctx.exit_requested` at the top also returns `Finished` without drawing. `initcoppers` is a bare `ret` under the standalone DIS (no copper, no split, no keys); `waitb`'s Scroll-Lock border and `kbhit`/`getch` are developer input, not ported. One iteration is one tick; the nominal mode-13h retrace is 70.086 Hz (registered as the fixed-rate mapping).

**Steps:**

- [ ] **Step 1: Write the failing rows test.** Pin the source's 16-bit arithmetic and the phase: `r` is a `u16` reset each frame and advanced by `ra` per row (the first in-frame wrap is at frame 327); `w` truncates toward zero (`sin=-255` → `w=1`, not 0); `w=200` maps to row 0; the selected rows for frames 0, 1, 2, 3, 99, 327, 999 and 4,443 are recorded from the source formula. Name it `the_selected_rows_are_the_sources_16_bit_phase`.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement `rows`.**
- [ ] **Step 4: Write the failing texture test.** Read `TWSTLOOP.INC`, walk its `twist<N>l`/`twist<N>r` labels and `mov ax,0NNNNh`/`mov ds:[di+N],ax` stores, reconstruct the 200×80-byte `TEXTURE`, and assert the port's constant equals it; pin the shape (8,000 stores, 55 distinct byte values, 121 rows with first byte `0x3F`, every byte ≤ 63). Name it `the_texture_table_is_the_shipped_twstloop`.
- [ ] **Step 5: Implement `TEXTURE` and the chunky draw.** The draw writes `TEXTURE[rows[y]]` into the current page at `origin + y*320`, each byte four times.
- [ ] **Step 6: Write the failing display/palette/cap/process tests.** `the_pages_alternate_origins_zero_and_131072` (even/odd frames, `start_addr`, `y_line_offset`, the last page-2 row at 195,071); `the_palette_is_the_eight_bit_write_masked_to_six_bits` (entry 63, 191, 192; not `from_vga`); `the_first_frames_are_the_measured_two_band_phase` (frame 0: rows 0..63 row 0, rows 64..199 row 199; frame f's boundary `ceil(64/(f+1))` for f 0..3); `a_part_run_of_twist_is_silent_and_numbers_itself_from_zero`; `twist_matches_the_reference_at_its_measured_start` (RGB comparison at `K0`, with the port-side `K0−1,K0,K0+1` supports; the reference is RGB-only); `twist_draws_exactly_4444_frames_and_holds` (the last drawn frame 4,443; 4,444 and 4,500 are the hold, not drawn).
- [ ] **Step 7: Implement the Part and register it.**
- [ ] **Step 8: Compare, register residuals, write the note.** Record the fixed-rate/no-overrun mapping, the dropped copper/Scroll-Lock/key input, and the RGB-only comparison in `docs/fidelity/twist.md` and `APPROXIMATIONS.md` (the input-model difference: the original exits on any key, the port on ESC/Ctrl).
- [ ] **Step 9: Commit.**

**Acceptance:** the row phase, the shipped texture, the page origins, the masked palette and the 4,444-frame cap are pinned by tests; `--part twist` runs silent at 320×200 and is compared with every filed keyframe at `K0` in RGB; residuals are 0 or registered.

---

### Task 5: HARD's vector engine

**Files:**
- Create: `bins/secondreality/src/parts/vault/hard/mod.rs` (module tree), `hard/math.rs` (rotation, projection, the 16-bit `crotprojlist`), `hard/poly.rs` (shoelace, easy lists, the clipper), `hard/span.rs` (the 128-row opaque compositor)
- Modify: `bins/secondreality/src/parts/vault/mod.rs` (module)
- Test: unit tests in the four files

**Interfaces:**
- Consumes: `glenz::math::{matrix_yxz, Matrix}` (byte-identical `VMATH.ASM`/`MATH.ASM`), `glenz::tables::{SIN, COS, SIN1024}` (identical `VMATHSIN.ASM`/`SIN1024.INC`). Do **not** reuse `glenz::math::rotate`, `project`, `poly::cross/build`, or `span::Compositor` — HARD's are forks with different stores, a different hidden formula and a different destination.
- Produces, consumed by Tasks 6–7:
  - `pub struct Projection { pub mul_x: i32, pub mul_y: i32, pub add_x: i16, pub add_y: i16, pub min_z: i32 }`; `pub const HARD_PROJECTION: Projection = Projection { mul_x: 240, mul_y: 200, add_x: 160, add_y: 64, min_z: 512 };` (`VDATA.ASM:30-36`).
  - `pub struct PVert { pub x: i16, pub y: i16, pub flags: u16, pub z: i32, pub raw_x: i16, pub raw_y: i16 }` — HARD's `projlist` stores the raw x at `+12` and raw y at `+14`, the unclamped z at `+8`, flags at `+4` (16 = z clamped, 8/4 = y out of window, 2/1 = x out of window; the window is 0..100 in `VDATA.ASM:14-19`, and only the dead `cpolylist` reads it).
  - `pub fn rotate(dst: &mut Vec<[i32; 3]>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3])` — **HARD's axis stores**: X = `(m12*x + m14*y + m16*z) >> 15 + add_x`, Y = `(m6*x + m8*y + m10*z) >> 15 + add_y`, Z = `(m0*x + m2*y + m4*z) >> 15 + add_z` (`VEC.ASM:105-180`: the first sum is pushed, the third remains in `ecx`, then `pop edx` takes the second and `pop eax` the first; `shld ecx,ebx,17` is `>>15` of the 64-bit sum).
  - `pub fn project(dst: &mut Vec<PVert>, src: &[[i32; 3]])` — 32-bit `imul` by 200/240, `idiv` by `max(z,512)` (signed), quotient low 16 + 64/160 (16-bit wrapping), raw x/y stored, stored z unclamped.
  - `pub fn rotate_project(dst: &mut Vec<PVert>, src: &[[i32; 3]], m: &Matrix, add: [i32; 3])` — `crotprojlist` (`VEC.ASM:250-330`): the same rotation, then a **16-bit** `imul word ptr`/`idiv bx` using the low 16 bits of z, no raw fields and no flags; this is a different projection from `project` and must be implemented separately.
  - `pub fn shoelace(points: &[(i16, i16)]) -> i32` — `checkhiddenbx`'s full shoelace (`VEC.ASM:369-400`): for each edge, `(x_i − x_{i+1}) * (y_i + y_{i+1})` as a signed 16-bit multiply into a 32-bit wrapping accumulator; the sign is the visibility (positive visible). This is not `glenz::poly::cross`.
  - `pub struct PolyDef { pub colour: u16, pub points: &'static [u16] }`, `pub struct Poly { pub colour: u16, pub points: Vec<(i16, i16)> }`, `pub enum FaceMode { Norm, Hide }`.
  - `pub fn easy_list(defs: &[PolyDef], verts: &[PVert], mode: FaceMode) -> Vec<Poly>` — `ceasypolylist` (`VEC.ASM:582-622`): build each list from the 16-byte records, drop a repeated neighbour and the closing duplicate, call the shoelace; `Norm` keeps the polygon's low colour byte, `Hide` sets a hidden face's colour word to 0. There is **no colour allocator and no DAC shading** in HARD (`demo_glz` is dead; `demo_norm` is a no-op).
  - `pub fn clip_easy_list(defs: &[PolyDef], verts: &[PVert], mode: FaceMode) -> Vec<Poly>` — `cclipeasypolylist` (`VEC.ASM:833-902`): clip each edge against `clipz = 4000` in 2-D using the raw x/y, project the intersection at z=4000, dedupe identical adjacent points, drop empty polygons, invert the carry (`cmc`), run the shoelace, then `mode`.
  - `pub struct Compositor { pub pixels: [u8; 64000] }` with `Compositor::new()`, `pub fn add(&mut self, polys: &[Poly])`, `pub fn paint(&mut self)` — `VNEW.ASM`: `NROWS = 128`, rows 0..127 only (`:162,341`), opaque `rep stosb` fill (no background OR), x clamp 1..319, XOR boundary colours, sentinels `(63999,0)`, `(-1,0)`. The band is **not cleared** by `cglenzinit`; `paint` leaves previous pixels between polygons.
- The source's `cglenzinit/done/polylist` are `__newgroup(0/2/1)` wrappers; the port's part calls `Compositor::add`/`paint` directly.

**Steps:**

- [ ] **Step 1: Write the failing rotation/projection tests.** `hard_rotation_takes_x_from_words_12_14_16_and_z_from_0_2_4` — `Matrix([32767,0,0, 0,32767,0, 0,0,32767])` and point `(1000,2000,3000)` must give `(2999,1999,999)`, while GLENZ's mapping would give `(999,2999,1999)`. `hard_projection_uses_240_200_160_64_and_min_z_512` — `(1000,2000,4000)` → X 220, Y 164, raw (1000,2000), z 4000; `(1000,-1000,100)` → z clamped to 512 for the divide, stored z 100, X 628, Y −326, flags **0x16** (16 z-clamp | 4 y below the 0..100 window | 2 x above it); the clipper's z=4000 projection of the same raw y would give Y 14, which is a different path. `rotate_projection_is_the_16_bit_crotprojlist` — a point with a z whose low 16 bits differ from z and a product that overflows 16 bits must match the 16-bit `imul`/`idiv`, not the 32-bit path.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the maths.**
- [ ] **Step 4: Write the failing shoelace/list tests.** `the_shoelace_is_the_full_signed_sum` — a square and its reverse (positive/negative), a quad whose first-three cross has the opposite sign from the full shoelace, and a wrapping-overflow case. `the_easy_list_is_index_only_and_drops_duplicates` — repeated neighbour and closing duplicate, `Norm` keeps the low byte, `Hide` zeroes a hidden face. `the_clipper_clips_at_4000_and_projects_the_intersection` — an edge crossing z=4000, the raw-x/y intersection, the dedupe, the inverted carry, and an empty clipped polygon.
- [ ] **Step 5: Implement the shoelace, the easy list and the clipper.**
- [ ] **Step 6: Write the failing compositor tests.** `the_opaque_compositor_paints_only_128_rows_and_never_clears` — a polygon spanning rows 120..140 leaves rows ≥128 untouched, a second polygon over the first leaves the uncovered pixels, the x clamp is 1..319, the XOR boundaries and the `63999` sentinel behave, and a direct span walk over the same edges matches byte for byte.
- [ ] **Step 7: Implement the compositor.**
- [ ] **Step 8: Commit** the engine and its tests.

**Acceptance:** the rotation's axis permutation, the two projection widths, the full shoelace, the clipper and the 128-row opaque compositor are each pinned by a test that fails on the GLENZ implementation; no GLENZ colour allocator or `bgpic` is used.

---

### Task 6: HARD's dots and its first half

**Files:**
- Create: `assets/HARD/FCRZ.U` (the 64,000-byte raw picture, copied from `upstream/SecondReality/HARD/FCRZ.U`), `bins/secondreality/src/parts/vault/hard/dots.rs` (the FCRZ extraction, the dot engine, the grid table, the geometry), `bins/secondreality/src/parts/vault/hard/part.rs` (the pre-roll, the letterbox, modes 0..4, the palettes, the Part)
- Modify: `assets/PROVENANCE.md` (the FCRZ.U row), `docs/fidelity/asset-inventory.md`, `xtask/src/assets.rs` and `xtask/tests/assets.rs` (the row count and decode totals), `bins/secondreality/src/parts/vault/mod.rs`, `bins/secondreality/src/parts.rs` (registry row `("hard", ...)`), `bins/secondreality/src/shell.rs` (music-coverage row `("hard", None)`), `bins/secondreality/src/parts/minvball.rs` (`MsRand` and its `new`/`next` become `pub(crate)`), `docs/fidelity/hard.md`, `bins/secondreality/tests/vault.rs`
- Test: unit tests in `dots.rs`/`part.rs`; the process tests in `tests/vault.rs`; `cargo xtask assets verify`

**Interfaces:**
- Consumes: Task 2's `reference/parts/hard/` and `K0`; Task 5's engine is not needed until Task 7; `crate::parts::minvball::MsRand`; `assets/HARD/FCRZ.U`.
- Produces, consumed by Task 7:
  - `pub struct Dot { pub x: i16, pub y: i16, pub vx: i16, pub vy: i16, pub life: u8, pub colour: u8, pub last: u16 }` and `pub fn add_dot(dots: &mut Vec<Dot>, ax: i16, ay: i16, x: i16, y: i16, colour: u8, slot: &mut u16)` — `adddot`'s six words (`B.C:84-98`): x/y scaled by 64, velocity, colour + (66<<8), last position.
  - `pub fn dodots(dots: &mut Vec<Dot>, background: &mut [u8; 64000], screen: &mut [u8])` and `pub fn dodots2(...)` — `BA.ASM:18-53,75-171`: restore the previous pixel from the background, advance, `sar 6`, clip y≤127/x≤319, draw, and on life 0 write the colour into the background and mark `0x100`; `dodots2` does not write the background. The screen argument is the visible A2D0 band slice (`screen[11520..52480]`), not the whole 64,000-byte screen.
  - `pub fn do3dots(...)` — `BA.ASM:224-280`: the part-2 grid dots, the two-row shadow clear, the signed `jg` skip of z > 3000, `depthcol[z]`, the 16-bit `bp` z load, and the negative-low-word guard (a model for an unreachable branch; Task 7 consumes).
  - `pub fn dots_from_fcrz(image: &[u8; 64000]) -> [Vec<u8>; 4]` — `BDO.C:12-41`: four 40-row bands, byte records `(value, 0, x-160, y-20)` for `0<value<4`, zero-terminated; the spawners read the byte streams (`dotsp += 4`), so the return is bytes, not live `Dot`s; the measured counts are **859, 690, 722, 1,071**.
  - `pub const GRID1: [i16; 200]` — the 400-byte `_GRID1.TMP` table, with the last word measured against `B.EXE` (the one-byte difference: `0x4F` in the EXE, `0x00` in the file).
  - `pub struct Hard { ... }` with `Hard::new()` and `impl Part`; `pub const HARD_NAME: &str = "hard"`.
- The source's order (`B.C:131-202`): `dis_partstart`; mode 13h; `waitb`; white 64,000 bytes; four `waitb`; the row-100 wipe (one pixel per x, a `waitb` every 16 columns); the accelerating fill from row 100 down (one `waitb` per iteration); **70** `waitb`; then `defpal` zero, `loadpal(defpal,768)`, the border write, the five `memset` bands, `defpal[0/254/255]`, `loadpal(defpal,768)`; `rows[a]=a*320`; `background`; `dotspnt=dots1`; the optional argument is not ported (the vault runs without arguments and the source's debug strip is overwritten at `:200-201`). The pre-roll's wait counts are source-determined; the port models them as phases so that frame 0 is the first wiped frame and the mode sequence starts at the measured `K0` offset.
- The source's state machine (`B.C:203-324`): `count++`, `setborder(255)`, `waitb`, palette updates, `setborder(0)`, `dodots`/`dodots2`, `switch(mode)`. This task implements modes 0..4 — 34 frames of `dodots` on `dots1`; four 73-frame logo phases (`part1init` at count 0, `part1` after, `NEXTMODE` at 72); four one-frame dot rotations then kill-all; 42 dot frames; 102 mode-4 frames (the moving 66-entry band, the forced `pal[189]=pal[1]=63` (`B.C:280-286`, not 251/3), the background rebuild, the dead `count>400` branch) — and ends at the mode-5 boundary. Modes 5..6 and the final `default` are Task 7's; until then `frame` returns `Finished` when mode 4 would advance, and a test pins that milestone.
- The visible screen is 320×200: rows 0..34 = index 255, row 35 = 254, rows 36..163 = the 128-row work band, row 164 = 254, rows 165..199 = 255; `defpal` entry 0 `(0,0,20)`, 254 `(45,45,45)`, 255 `(0,0,0)`. The work band aliases `A2D0:0000` at visible rows 36..163. View 320×200, origin 0, `y_line_offset=Some(320)`; the dots' clip is y≤127/x≤319.
- The palette uploads are partial: `part1` uploads **96 bytes (32 entries)** per frame (`BP.C:59`), the others 768. The port keeps a `[u8; 768]` DAC mirror and applies the source's byte offsets, then `ctx.set_palette` with the mirror (or `set_palette_entry` for the touched range); it must not normalise the 96-byte upload to 768.

**Steps:**

- [ ] **Step 1: Curate the asset.** Copy `FCRZ.U` to `assets/HARD/FCRZ.U`, add the provenance row with its SHA-256, update the inventory and the asset-count assertions; `cargo xtask assets verify`.
- [ ] **Step 2: Write the failing data tests.** `the_dot_streams_are_the_bdo_extraction` — read `assets/HARD/FCRZ.U`, run the four-band extraction, assert the counts 859/690/722/1,071, the record layout, the zero terminator, and (as a cross-check) that `B.EXE`'s four embedded streams at offsets 55875/59315/62082/64977 match except the measured terminator bytes. `the_grid_table_is_the_shipped_grid1` — read `upstream/SecondReality/HARD/_GRID1.TMP`, assert 200 words and the last word as measured against `B.EXE`; record the discrepancy in the note.
- [ ] **Step 3: Run; fail. Implement the extraction and the table.**
- [ ] **Step 4: Write the failing dot-engine tests.** `the_dot_engine_leaves_the_66_frame_trail` — a synthetic dot at a known position; the restore/advance/clip/background rule over 66 frames, the `0x100` dead marker, y≤127/x≤319; `dodots2` leaves trails. `the_shadow_grid_dot_skips_positive_z_and_models_the_negative_word` (Task 7 consumes; pin the rule now) — the signed `jg` skips positive z > 3000; the negative-low-word branch is unreachable in part 2 (the projected z′ is full ≈1134..4366, drawn ≤3000, always with a positive low word) and its zero is a registered model (A51); the 16-bit `bp` z load.
- [ ] **Step 5: Implement the dot engine.**
- [ ] **Step 6: Write the failing pre-roll/letterbox/palette tests.** `the_pre_roll_is_the_sources_tick_counts` — the four waits, the 20-column wipe, the fill's iteration count and the 70 waits, all derived from `B.C:139-162`; the white screen and the letterbox bands; `the_letterbox_and_defpal_are_the_sources`; `the_part1_palette_uploads_only_32_entries` — the byte offsets, `pal1colp`, `loadpal(...,96)`; `the_mode_four_band_palette_is_the_sources`.
- [ ] **Step 7: Implement the pre-roll, the letterbox and the palettes.**
- [ ] **Step 8: Write the failing mode tests.** `the_first_half_mode_sequence_is_the_sources` — counts 34, 4×73, 4, 42, 102; `NEXTMODE` (`count=-1`, `mode++`); the mode-4 dead `count>400`; `part1`'s spawners and the `MsRand` jitter (no burn: `B.C:122` is not a loop); the logo tables `dots1..4`.
- [ ] **Step 9: Implement modes 0..4 and the Part.** Register the name; update the shell's music table.
- [ ] **Step 10: Write the process tests and compare the early keyframes.** `a_part_run_of_hard_is_silent_and_numbers_itself_from_zero`; `hard_matches_the_reference_in_its_early_modes` — at each filed keyframe before mode 5, the `K0`-relative port frames `R-K0-1, R-K0, R-K0+1` compared with the reference RGB; the minimum residual recorded. The Task 7 milestone test pins that the part currently ends at the mode-5 boundary.
- [ ] **Step 11: Register residuals and commit.**

**Acceptance:** FCRZ.U is curated with provenance and the asset gates pass; the dot streams, grid table, dot engine, pre-roll, letterbox, palettes and modes 0..4 are pinned by tests; `--part hard` runs silent to the mode-5 boundary and matches the early filed keyframes at `K0` (0 or registered residuals).

---

### Task 7: HARD's grid, cube and full run

**Files:**
- Modify: `bins/secondreality/src/parts/vault/hard/part.rs` (modes 5..6, the default exit, `part2init`/`part2`/`part3init`/`part3`, `fadepal`), `hard/dots.rs` (the grid points, `do3dots`), `docs/fidelity/hard.md`, `bins/secondreality/tests/vault.rs`
- Test: unit tests in `part.rs`/`dots.rs`; the process tests

**Interfaces:**
- Consumes: Task 5's `rotate`, `project`, `rotate_project`, `easy_list`, `clip_easy_list`, `Compositor`, `PVert`, `FaceMode`, `HARD_PROJECTION`; Task 6's `Hard`, `Dot`, `do3dots`, `GRID1`.
- Produces: the complete `Hard` part through modes 5..6 to the `default` exit (`mode=-1`), with the source's 500-frame part-2 grid and 800-frame part-3 cube; the full run retires on `dis_exit`/ESC or after the last mode.
- Part 2 (`BP.C:124-215`): `part2init` builds the 10×10×10 lattice shell (`(x-5)*100` for cells with any index 0 or 9), **488 points** (10³−8³; `gridpoints[0]=(a-1)/3`), `zp=3500`; each frame `cmatrix_yxz(rx,ry,rz)`, `csetmatrix(matrix,0,0,zp)`, `crotprojlist(points3,gridpoints)`, `do3dots(points3,vram)`; after count>1, `rx+=9; ry+=10; rz+=11; zp-=10` down to 2000; ends at `count>=500`. `part2init`'s `b` is uninitialised for entries ≥1000 (`BP.C:130`); reproduce the source's 16-bit values (the previous iteration's last value), not a clean initialisation.
- Part 3 (`BP.C:220-299`): `cubepoints` **−510..410**, `cubepolys` colours `0x4001,0x4002,0x4004,0x4008,0x4010,0x4020`, `cubepolyss` all `0x4040` with indices +8; per frame the `count>=800` and (unreachable) `muscode(0xff)==0xff` checks, `ct2+=4`, `lix=sin1024[ct2&1023]/2+160`, `liy=sin1024[(ct2+256)&1023]/2+64`, `cglenzinit`, the cube's rotation/projection, the second copy projected toward `(lix,liy)` with `lz=5000`, `z'=3000`, `ceasymode(0)`/`clip_easy_list`/`cglenzpolylist` for the magnified copy, `ceasymode(1)`/the real cube, `cglenzdone`, `rx+=9; ry+=10; rz+=11`, and `if(count<20) fadepal(pal,pal2,count*16)`. The band is **not cleared** between frames.
- `fadepal` (`BA.ASM:174-215`): `src_channel * dl / 256` via the high byte of an 8-bit multiply, plus `20*(255-dl)/256` to blue; the port reproduces the 8-bit multiply's `imul` high byte.
- The mode sequence after the pre-roll: 34 + 4×73 + 4 + 42 + 102 + 500 + 800 = 1,774 source frames; the artifact adds **two static display ticks** at the mode-5 boundary (A48: part2init's picture is the mode-4 hold, the first visible grid frame is two displays after the source-derived 954; 956 is that frame; the overrun's position is not measured, only its total cost). The port's full run is 1,903 frames. Mode 7/default ends the part. The `0xff` cut is unreachable with the vault's single start event (the note registers the substitution).
- The final `if(!dis_indemo())` mode restore is not ported; the shell/window handles presentation. No `drains_after_exit`.

**Steps:**

- [ ] **Step 1: Write the failing part-2 tests.** `the_grid_is_the_ten_cube_shell` — 488 points, the shell criterion, `(x-5)*100`; `part2init_reads_the_uninitialised_b` — the exact 16-bit values for entries 1000..3099 given the source's loop order; `the_part2_projection_is_the_16_bit_crotprojlist` — a point whose low-16 z differs, through Task 5's `rotate_project`.
- [ ] **Step 2: Run; fail. Implement the grid and part 2.**
- [ ] **Step 3: Write the failing part-3 tests.** `the_cube_geometry_and_colours_are_the_sources` — −510..410, the six colour words, the +8 copy, the projection `(x-lix)*5000/(z+2000)+lix`, `z'=3000`; `the_part3_mode_selects_hide_and_norm` — `ceasymode(0)` zeroes hidden faces, `ceasymode(1)` keeps them; `the_band_is_not_cleared_between_frames`; `fadepal_is_the_high_byte_multiply` — a known palette pair and `dl`, including the blue bias.
- [ ] **Step 4: Implement part 3 and `fadepal`.**
- [ ] **Step 5: Write the failing full-sequence tests.** `the_full_mode_sequence_is_the_sources` — the 1,774-frame count, the mode-7/default exit, the part retiring; `the_0xff_cut_is_unreachable_with_one_start` — no second key, `muscode(0xff)` never fires; `hard_draws_the_whole_effect_and_holds`.
- [ ] **Step 6: Implement modes 5..6 and the exit; run the full process test.**
- [ ] **Step 7: Compare every filed HARD keyframe.** At each reference index `R`, dump `R-K0-1, R-K0, R-K0+1`, compare RGB, record the minimum and its offset; register every non-zero residual and the candidate cause (palette upload phase, dot background, the grid table's last word, the `b` value) in `APPROXIMATIONS.md`/`docs/fidelity/hard.md`.
- [ ] **Step 8: Commit.**

**Acceptance:** the full HARD effect runs silent from the pre-roll through part 3 to its own exit; the mode counts, the uninitialised `b`, the signed-z/negative-word depth guard, the cube's two passes and `fadepal` are pinned; every filed keyframe is compared at `K0` with its residual recorded.

---

### Task 8: The vault menu

**Files:**
- Create: `bins/secondreality/src/vault_menu.rs` (the pure menu state and its drawing)
- Modify: `bins/secondreality/src/main.rs` (`mod vault_menu;` and a `new_sequence_for_part` helper if needed), `bins/secondreality/src/cli.rs` (`vault: bool`, `--vault`, validation, usage), `bins/secondreality/src/window.rs` (the menu loop and the return-to-menu run outcome), `bins/secondreality/src/parts/vault/mod.rs` (the `VAULT` table from Task 3), `bins/secondreality/tests/shell.rs` (the headless refusal)
- Test: unit tests in `vault_menu.rs`, `cli.rs` and `window.rs`; `tests/shell.rs`

**Interfaces:**
- Consumes: `parts::vault::VAULT: [(&str, &str); 3]`; `sr_assets::load_path` + `sr_assets::font::Font::from_image` with `assets/ALKU/FONA.UH`, cell `(20, 30)`, first `'A'`; `sr_engine::compose::IndexedFrame`; `sr_engine::Palette`; the window's `present`/`streaming_texture`/`blit` helpers; `crate::new_sequence`.
- Produces:
  - `pub enum MenuKey { Up, Down, Enter, Escape, Other }`, `pub enum MenuAction { Redraw, Choose(&'static str), Close }`, `pub struct Menu { selected: usize }` with `Menu::new()`, `pub fn key(&mut self, key: MenuKey) -> MenuAction` (Up/Down wrap 0..2; Enter chooses; Escape closes), and `pub fn draw(menu: &Menu, font: &Font, frame: &mut IndexedFrame)`.
  - `pub fn menu_palette() -> Palette`: black background, white title, grey entries, amber selection, dim-blue footer; all values 6-bit.
  - `--vault` in `Cli`; `validate` refuses it with `--headless`, `--part`, `--block`, `--dump-dir`, `--dump-frames`, `--frames`, `--run-seconds` and `--audio-dump` (`the vault menu needs a window; name a part with --part`), and allows `--scale`, `--aspect`, `--no-border`, `--no-audio`.
  - The window's menu loop: draw the 320×200 menu, poll Up/Down/Enter/Escape/Quit, and on `Choose(name)` build a run of `[name]` and present it with the existing pacing, sound-less; when the run is holding and ESC is pressed, return to the menu instead of closing; ESC in the menu closes the window. The normal (non-`--vault`) path is unchanged.
- The menu is a new, non-fidelity surface: a 320×200 indexed frame, the demo's own `FONA.UH` glyphs at 1:1, a title `SECOND REALITY` / `VAULT`, the three labels, a `>` marker on the selected row, and the footer `UP DOWN  ENTER  ESC`. The font's cell is 20×30; a space advances by 10 columns; glyph pixels are the strip's 2-bit values (0 = background, 1..3 = ink).

**Steps:**

- [ ] **Step 1: Write the failing menu tests.** `the_selection_wraps_up_and_down` (0→2→1→0), `enter_chooses_the_selected_part`, `escape_closes_the_menu`, `an_unhandled_key_redraws_nothing`; `the_menu_frame_is_three_entries_with_a_selected_row` — 320×200, the three labels present, the selected row's pixels differ from the same row unselected, and the frame is not blank. Name them exactly.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the menu state and drawing.** Draw into an `IndexedFrame` with `PaletteTimeline::new(menu_palette())`.
- [ ] **Step 4: Write the failing CLI tests.** `the_vault_flag_is_parsed`, `a_vault_run_refuses_headless`, `a_vault_run_refuses_a_part_or_block`, `a_vault_run_refuses_a_dump_or_audio_flag`, `a_vault_run_accepts_the_presentation_flags`. Add the flag to the usage string.
- [ ] **Step 5: Implement the CLI and validation.**
- [ ] **Step 6: Write the failing window tests.** A pure reducer `vault_key(key_of...)`/the menu's key mapping; the return-to-menu predicate (a holding run plus ESC returns to the menu, not closes); the normal run's ESC/close predicates unchanged (the existing tests must still pass). Name the new ones `the_menu_keys_map_to_the_menu_actions`, `an_escape_on_a_held_vault_part_returns_to_the_menu`.
- [ ] **Step 7: Refactor `window::run`.** Extract the sequence-presenting loop into a helper that returns `RunEnd { Close, Menu }`; the normal path passes `Menu` unreachable. Add the `--vault` outer loop: menu frame, key handling, run, return. Do not change the normal path's status line, pacing or ESC semantics.
- [ ] **Step 8: Run it.** `cargo run --release --bin secondreality -- --vault`; select each of GRID, HARD and TWIST, confirm the picture, ESC returns to the menu, ESC exits, and the window title in the menu reads `Second Reality — vault`. Record the manual check in the report; if the session has no display, state that and rely on the tests plus the user's later run.
- [ ] **Step 9: Commit.**

**Acceptance:** `--vault` is window-only, refusals are tested, the menu's state and frame are tested, and the three parts are selectable and return to the menu on ESC; normal runs are unchanged; the full suite and clippy are clean.

---

### Task 9: Close-out — the register, the record and the evidence

**Files:**
- Modify: `PROGRESS.md`, `docs/decisions.md` (D39: the vault's boot and standalone-DIS semantics; D40: the vault menu and the silent run), `docs/fidelity/{grid,hard,twist}.md` (the finished notes), `docs/fidelity/APPROXIMATIONS.md` (the silent run, the dropped key waits, the TWIST fixed-rate mapping and any measured residuals), `docs/fidelity/bugs.md` (the preserved source bugs the pictures depend on: GRID's uninitialised `d`, HARD's uninitialised `b`, the mode-4 189/1 palette quirk and the part-2 fade's negative `a*16` (the unsigned clamp makes `dl` 255, so it dims only 255/256)), `docs/fidelity/reference-indices.md` §24 (the final capture facts), `CLAUDE.md` (the vault commands), `docs/plans/08-vault.md` (the self-review notes), `assets/PROVENANCE.md`/`docs/fidelity/asset-inventory.md` (FCRZ.U)
- Test: the full workspace suite; both clippy gates; `cargo xtask reference verify --tree reference`; the three vault comparison tests

**Interfaces:**
- Consumes: every earlier task; the filed captures and their `K0`s.
- Produces: the phase's durable record and the evidence table; no new runtime code.

**Steps:**

- [ ] **Step 1: Re-run the evidence at the committed state.** For each part, the capture commands and the keyframe comparisons; record every residual (0 or registered) with the command that produced it. Re-run the full workspace suite and both clippy commands; record the counts.
- [ ] **Step 2: Write the decisions.** D39: the vault's artifacts (`P.EXE`/`B.EXE`/`T.EXE` + `DIS.EXE`), the route-A capture recipe, the one-start-key semantics and the silent run. D40: the window-only `--vault` menu, the `--part` selection for headless/tests, and the Phase 9 deferrals (settings screen, loop mode).
- [ ] **Step 3: Write the register.** Every substitution and residual in `APPROXIMATIONS.md` with its measured difference; the preserved bugs in `bugs.md`; the notes' numbers reproducible by named commands (D25).
- [ ] **Step 4: Update `CLAUDE.md` and `PROGRESS.md`.** The vault commands, the parts' standalone/silent behavior, the menu; the phase's row, evidence and "where to pick up".
- [ ] **Step 5: Self-review the plan and the phase.** The `docs/plans/08-vault.md` self-review notes: spec coverage (success criterion 2, Phase 8's exit), step/type consistency, the Review Focus tests, proportion.
- [ ] **Step 6: Commit the close-out.** The tag is withheld until the final whole-branch review is clean (Phase 1–7 practice).

**Acceptance:** the phase's evidence is recorded and re-runnable, the decisions and registers are written, the suite and clippy are clean, and the close-out commit is ready for the final whole-branch review.

---

## Self-review notes (for the controller, not the implementers)

- **Spec coverage.** Phase 8's exit ("GRID, HARD, TWIST playable standalone and verified") maps to Tasks 3–7 (the parts and their comparisons) and Task 8 (the D2 menu); the capture route is Task 1/2. No spec requirement is left without a task. Phase 9's settings screen and loop mode are deliberately not here (D40).
- **Type consistency.** `PVert`, `Projection`, `FaceMode`, `Compositor`, `PolyDef` are defined in Task 5 and consumed by Task 7; `Hard`, `Dot`, `do3dots` and `GRID1` are defined in Task 6 and consumed by Task 7 (`dots_from_fcrz` is Task 6's spawner input); `VAULT` is defined in Task 3 and consumed by Task 8; `K0` is defined in Task 2 and consumed by Tasks 3, 4, 6 and 7.
- **Review Focus tests.** Each of the five lines is pinned: the 16-bit bugs in Tasks 3, 6 and 7; the start/cue substitutions in Tasks 3, 4, 6 and 7; the display mapping in Tasks 3, 4 and 6; the generated loops/tables in Tasks 3, 4 and 6; the integration traps in Tasks 3, 4 and 8.
- **Proportion.** The plan is the set of decisions the implementers cannot make alone; the algorithms stay in the cited source files, and the task briefs carry the exact values and tests.
- **Task 9's close-out.** The spec coverage (success criterion 2 and Phase 8's exit) and the type/coverage lines above were re-checked at the phase's tip. The menu frame test now asserts the `>` marker and the `VAULT` subtitle directly, the GRID comparison asserts its residual rows rather than only printing them, the committed `LINEBLIT.INC` is tied to the port by a test and the whole-word `d0` class by another, and the A51 z′ range is corrected to the full/drawn domain above. The phase's evidence and the deferred minors are recorded in `PROGRESS.md`, `APPROXIMATIONS.md` A40–A52 and `bugs.md` §6–§9.

---
