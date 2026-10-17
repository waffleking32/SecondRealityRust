# Phase 4 — Music-synced tier A (ALKU, PAM, DDSTARS, DOTS, MNTSCRL, RAYSCRL) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the six music-synced tier-A parts — ALKU, PAM, DDSTARS, DOTS (`MINVBALL.EXE`), MNTSCRL (`FOREST`) and RAYSCRL (`WATER`) — so each runs against the reference's own frames, each waits on the music exactly as the original does, and the run is playable through the demo's own blocks.

**Architecture:** Six new part modules under `bins/secondreality/src/parts/`, each a transliteration of its upstream source into the existing `Part` API (`crates/sr-engine/src/part.rs`), drawing into the indexed `Surface` through the `View`. One shared decoder (`parts/poslist.rs`) for the per-column position tables MNTSCRL and RAYSCRL both use. The music waits become *ticks* — every busy-wait in the originals becomes a `frame()` that returns `Continuing` without drawing until its cue arrives, because the project does not spin. Shell wiring adds the six to `PART_ORDER`'s blocks and `music_for_part`, and the acceptance is `xtask compare` against the four filed keyframes per part.

**Tech Stack:** Rust 2021, `crates/sr-engine` (part API, surface/view, copper, palette), `crates/sr-assets` (Origin/IFF/RIX3/ANI readers), `xtask` (reference capture/compare, music trace), SDL2 only in the binary.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §2.2 (part sequence) and §10 (Phase 4's exit criterion: "ALKU, PAM, DDSTARS, DOTS, MNTSCRL, RAYSCRL verified"). The phase's binding rules are in `docs/decisions.md` (D4, D7, D8, D15, D16, D17, D19, D21, D22, D24, D26–D29), `docs/fidelity/APPROXIMATIONS.md` (A1, A3, A4, A9, A12, A15, A16) and `CLAUDE.md`.

## Global Constraints

- **The reference is the authority; the object and the shipped release are the tiebreakers.** The vendored tree is a development snapshot (D24); where a reference frame disagrees with a faithful tree port, D24's procedure decides and the divergence is recorded.
- **Port the algorithm, not a resemblance.** Fixed-point arithmetic, table values and rounding are transliterated, including the originals' *bugs* (D7). Every approximation is registered in `docs/fidelity/APPROXIMATIONS.md` with its measured difference.
- **Assets are read in their original formats.** No conversion. `reference/` and `upstream/` are input-only; nothing is written into them.
- **6-bit palette internally**, widened only at upload (`dac_expand(v) = (v*259+33)>>6`, D16).
- **Never spin-wait.** The logic tick is fixed at 70.086 Hz; a part that waits for a cue returns `Continuing` and is called again next tick. An original's `while(!condition);` is a tick-consuming wait here, never a loop.
- **The view's semantics are D16's**, not the original register names: `start_addr` is a pixel address (`u32`), `x_offset` is within a line, a split's lower view reads origin 0, `y_line_offset` defaults to the surface stride.
- **`sr-engine` keeps zero runtime dependencies; `sr-audio` keeps zero; `sr-assets` may depend on `sr-engine` only.**
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29).
- **Part names are the demo's binaries**: `alku`, `pam`, `minvball` (source dir `DOTS`), `ddstars`, `mntscrl` (source dir `FOREST`), `rayscrl` (source dir `WATER`). `PART_ORDER` already uses these.
- **Tests and clippy are the gate**: `cargo test --workspace` green, zero warnings, both clippy commands clean. Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

Five input classes or failure modes the spec implies but no single task's tests would otherwise catch. Each is pinned by a named test in the task that owns the code.

1. **A cue that never arrives must not hang the run silently.** The originals' waits are exact (`dis_musplus == -11` in MNTSCRL/RAYSCRL; `-4 < musplus < 0` in DOTS; `dis_sync() < N` in ALKU/PAM); a port one tick off the cue waits for ever. A part waiting on a cue must still honour `dis_exit` every tick, and its test must drive a sequence in which the cue is *skipped* and assert the part exits rather than loops.
2. **A busy-wait in the original is a tick-consuming wait here, and it must not starve the other parts or the exit key.** RAYSCRL's second stage spins on `dis_sync`/`musrow` with no `waitb` (`DEMO.PAS:135-143`), and ALKU's scroll waits spin on `frame_count` (`MAIN.C:399-401`). The port's `frame()` must return `Continuing` promptly; a test asserts the exit flag is observed while the part is still waiting.
3. **The parts' positions are music-counted, and the phase has no trace for some of its windows.** ALKU and PAM wait on `dis_sync`, and no `SECOND 1` trace exists; DOTS/RAYSCRL's cue windows are past `second3`'s end; DDSTARS's are past `hiddenu`'s. The tasks that need a trace file one (Task 1), and every cue test states which trace it uses or that it is synthetic.
4. **Where the shipped build differs from the tree, the port follows the release (D24).** DDSTARS's text-clip tests are assembled out in the tree (`STARS.ASM:692-696` etc.) yet the reference shows text; ALKU's linked `hzpic` differs from `HOI.U` in palette entries 1–3; `FONA.UH` is shifted one byte from what the part links (A9). The note must say which artifact each pixel was reconciled against.
5. **The original's deliberate arithmetic bugs are the picture.** RAYSCRL's palette fade advances three bytes per entry and doubles the *next* colour's red (`DEMO.PAS:125,174`), its text insert index is off by one band (`:184`), and its `move` reads one byte past the array (`:183`); MNTSCRL writes one byte past `fbuf` (`READ2.PAS:119`); DOTS's off-screen branch overwrites its shadow offset with a pixel value (`ASM.ASM:194-196`). A "fixed" port fails the diff; the test must pin each bug.

---

## The measured ground truth

Every row below is from the source survey; **the implementer re-derives each against the file**, because the plan is an argument and the tree is the evidence.

| Part | Source | Stages / exit | Music calls | Assets | Reference keyframes |
|---|---|---|---|---|---|
| ALKU | `ALKU/MAIN.C` 542 lines, `ASMYT.ASM`, `COPPER.ASM`, `TWEAK.ASM` | 3 cards gated by `dis_sync()<1/2/3`; picture fades in over 128 frames while `do_scroll(2)` runs; scroller for `a<320`; exit on `dis_exit` or `a>=320` | `dis_sync()<1,<2,<3,<4`, and `<4+tptr` in the text batches; three copper routines | linked `hzpic` (`HOI.U`-family, 640×200) and `font` (`FONA.UH`, shifted, A9) | 900, 1719, 2719, 5859 (320×400, `.rgb` only) |
| PAM | `PAM/OUTTAA.C` 67 lines, `ASMYT.ASM` (`ulosta_frame`), `COPPER.ASM` | `dis_sync()<10`; 45 loop steps, 40 frames drawn; exit on `dis_exit` or `f>=45` | `dis_sync()<10` only | `OUT.ANI` (42 delta frames), `PAL.PAL` | 7710, 7760, 7830, 7900 (320×200; `.idx` at 7900) |
| DDSTARS | `DDSTARS/STARS.ASM` 803 lines, `KOE.ASM` | one unbounded loop; exit only on `dis_exit`; 3200-frame stage marks | none (`dis_waitb` only); shell cues MUSIC0@70 | `TEXTS.16` (`_textpic`, 2 planes) | 1900, 3000, 5000, 8000 (320×400, `.rgb` only) |
| DOTS | `DOTS/MAIN.C` 291 lines, `ASM.ASM` 250 | palette fade-in 65 steps; main loop `frame<2450`; exit on `dis_exit` or music `-4 < musplus < 0` | `dis_musplus` (in demo only); `dis_waitb`; `dis_indemo` | none (runtime-generated) | 10900, 11200, 11800, 12400 (320×200), filed as `reference/parts/minvball/` |
| MNTSCRL | `FOREST/READ2.PAS` 224 lines, `ROUTINES.ASM` | 64-frame fade-in; font build; wait `musplus>=0`; 127-frame pre-roll; main loop; exit on `dis_exit` or fade when `musplus==-11` | `dis_musplus>=0`, `dis_musplus==-11` | `HILLBACK.CLX`, `O2.SCI`, `POS1-3.DAT` | 500, 1100, 1700, 2300 (320×200; `.idx` at 500) |
| RAYSCRL | `WATER/DEMO.PAS` 190 lines, `ROUTINES.ASM` | fade-in 127; wait `musplus>=0` then order change and `row>=16`; main loop; no frame bound; exit on `dis_exit` or `musplus==-11` | `dis_musplus>=0`, order change, `row>=16`, `-11` | `BKG.CLX`, `MIEKKA.SCI`, `WAT1-3.DAT` | 13000, 13500, 14000, 14500 (320×200; `.idx` at 13500) |

---

### Task 1: The traces Phase 4 needs, and their manifests

**Why first:** five of the six parts' cue behaviour is verified against a filed trace, and the existing four do not cover this phase's windows. Without these, Tasks 6–8's cue tests are synthetic only, and the phase's claim "the part waits where the original does" rests on nothing.

**Files:**
- Create: `reference/music/second1/` (trace + manifest, frames 148–6000: ALKU), `reference/music/second1b/` (frames 7600–7960: PAM), `reference/music/second3b/` (frames 10200–15050: DOTS and RAYSCRL), `reference/music/hiddenu2/` (frames 6500–8200: DDSTARS's 8000 keyframe)
- Modify: `docs/fidelity/music-clock.md` (§2's trace table, §1.7's spread), `docs/fidelity/reference-indices.md` if a window bound moves.

**Interfaces:**
- Consumes: `cargo xtask music trace` (Phase 3, `xtask/src/music.rs`), the trace-file format (`frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe`), and the manifest fields Task 3 recorded.
- Produces: four directories under `reference/music/`, each with the trace and its manifest; `music-clock.md`'s table naming all eight traces and the frames each covers.

**The window decision, made here rather than left to the implementer:** the four captures are separate because D19's deferred-read cost grows with the span and because each part's window is what the tests need. `second1` covers ALKU (404–5959); `second1b` covers PAM (7700–7920) — it starts at 7600 so the part's `dis_sync()<10` wait is visible before the first drawn frame; `second3b` covers DOTS (10300–12900) and RAYSCRL (12900–14900) with margin on both sides; `hiddenu2` covers DDSTARS's last keyframe and the stage marks around it. The 2900–10200 gap in `second3` (LENS/PLZPART, Phase 5) is deliberately not captured.

- [ ] **Step 1: Capture `second1`** with the Phase 3 recipe: `cargo xtask music trace --arg 1 --frames 148..6000 --out reference/music/second1` (read `xtask music trace --help` for the exact spellings; the recipe is the one that produced `second3`, `second5`, `hiddenu`).
- [ ] **Step 2: Capture `second1b`**: `--arg 1 --frames 7600..7960`.
- [ ] **Step 3: Capture `second3b`**: `--arg 3 --frames 10200..15050`.
- [ ] **Step 4: Capture `hiddenu2`**: `--arg U --frames 6500..8200`, covering DDSTARS's 8000 keyframe.
- [ ] **Step 5: Assert the manifests and the trace reader.** `cargo xtask reference verify --tree reference` must pass with the new files; the trace test (`crates/sr-audio/tests/trace.rs`) must pick the new traces up (it reads `reference/music/*` — check whether the test's list is explicit and extend it).
- [ ] **Step 6: Record the windows and the spread in `music-clock.md`**: the table gets each new trace's name, frames, module and anchor, and §1.7 states whether the new sessions' anchors agree with the 408 that Phase 3 measured (they must be reported, not assumed).
- [ ] **Step 7: Commit.**

**Acceptance:** `cargo xtask reference verify --tree reference` passes; `cargo test -p sr-audio --test trace` passes over the extended set; the note's table names every trace with its frame window.

---

### Task 2: The position-list decoder (`POS`/`WAT` format)

**Why shared:** MNTSCRL's `POS1-3.DAT` and RAYSCRL's `WAT1-3.DAT` are the same format — per-column command streams written by `DATGEN.PAS` and consumed by `ROUTINES.ASM`'s blitters. One decoder, one set of tests, two parts.

**Files:**
- Create: `bins/secondreality/src/parts/poslist.rs`, with `#[cfg(test)]` tests in the same file.
- Modify: `bins/secondreality/src/parts.rs` (`pub mod poslist;` — crate-internal).

**Interfaces:**
- Consumes: raw bytes (`std::fs::read`), the part's source data path (the `tunneli.rs` precedent: the part reads its own `.DAT` with `std::fs::read` and decodes inline).
- Produces:
  ```rust
  /// One column's destination-offset list: a u16 count followed by that many u16
  /// screen byte offsets, in the little-endian order the file carries.
  pub(crate) struct PosList { pub columns: Vec<Vec<u16>> }
  impl PosList {
      pub(crate) fn parse(bytes: &[u8]) -> Result<PosList, String>;
      pub(crate) fn column(&self, i: usize) -> &[u16];
  }
  ```
  The blitters stay in each part: MNTSCRL's is additive with a +128 bias and a transparent 0, RAYSCRL's is colour-keyed against the clean background, and the two are different enough that sharing them would hide the difference the notes must record.

**Steps:**
- [ ] **Step 1: Write the failing test** with a hand-built synthetic list: a two-column stream whose bytes are written literally in the test, asserting `columns` and the offsets. Include a truncated stream and assert `Err`, not a panic.
- [ ] **Step 2: Run it and watch it fail.**
- [ ] **Step 3: Implement `PosList::parse`** with the format the two `ROUTINES.ASM` files agree on (`FOREST/ROUTINES.ASM:21-35`, `WATER/ROUTINES.ASM:21-36`): u16 count, then `count` u16 offsets. Re-derive from both files; if they differ, the difference is a finding and the decoder takes the field that both read.
- [ ] **Step 4: Run the test; it passes.**
- [ ] **Step 5: Add the real-data test**: parse `assets/FOREST/POS1.DAT`, `POS2.DAT`, `POS3.DAT`, `assets/WATER/WAT1.DAT`, `WAT2.DAT`, `WAT3.DAT` and assert the column counts and the total position counts the survey measured — MNTSCRL 237×31 columns and 15,592 positions across three files; RAYSCRL 158×34 columns and 20,858 positions (6,176 + 6,145 + 8,537). If the totals disagree, the decoder is wrong or the survey is; the test prints which.
- [ ] **Step 6: Commit.**

**Acceptance:** the synthetic test fails without the parser and passes with it; the six real files parse with the measured totals.

---

### Task 3: MNTSCRL — the mountain scroller

**Files:**
- Create: `bins/secondreality/src/parts/mntscrl.rs`, `docs/fidelity/mntscrl.md`
- Modify: `bins/secondreality/src/parts.rs` (registry row), `bins/secondreality/tests/shell.rs` (the expected-music-row list), `bins/secondreality/src/shell.rs` (`music_for_part`)

**Interfaces:**
- Consumes: `PosList` (Task 2); `sr_assets::load_path` for RIX3; `PartCtx` (`set_palette`, `set_palette_entry`, `surface`, `frame`, `exit_requested`, `music`).
- Produces: `pub struct Mntscrl` implementing `Part`, registered as `"mntscrl"`.

**The transliteration, from `FOREST/READ2.PAS` (re-derive each line):**
- **Init**: mode 13h equivalent (320×200 surface); DIS start; load `O2.SCI` into a 640×32 strip, add 128 to every source byte (the +128 bias, `:119`); load `HILLBACK.CLX`; rebase the background from +778 to +0 and blit it; build the fade tables; force palette bands 0–31 and 128–159 black; build the font from source columns 0–132 into font columns 104–236.
- **The pre-roll and the wait**: after the 64-frame fade-in and the font build, the original busy-waits `dis_musplus() >= 0` (`:153-159`). The port: `frame()` returns `Continuing` without drawing until `musplus() >= 0`, and still returns `Finished` if `exit_requested`.
- **The 127-frame pre-roll** (`:165-182`): advances the scroll 42 columns; its "fade" is a no-op in the original (the two tables are equal) and only re-uploads entries 0–176 on odd frames — **preserve both facts**: upload on odd frames, and do not invent a fade.
- **The main loop** (`:193-224`): one tick = `waitb`; poll `musplus`; if fading, 65 DAC steps then `Finished`; else `scr(sss)` with `sss` rotating 0,1,2; test exit.
- **`scr(i)`** (`:86-88`): blit one of the three position lists into the packed framebuffer with the additive control — font byte 0 is transparent, nonzero source bytes get +128 (so the O2 value 7 becomes 135); `scr(2)` additionally shifts the 237×31 font left one byte, appends `fbuf[ff*640+scp]` at column 236, and increments `scp` toward 639.
- **The cue**: fade when `dis_musplus() == -11` **exactly** (`:196-203`), which is row 53 when `zplus==1`. Do not use `< -10` or `<= -11`.
- **Bugs to preserve**: the `for x := 0 to sizeof(fbuf)` write one byte past `fbuf` (`:119`); the dead `veke=2800` bound (the original never increments `frame`); the palette bands forced black.

**Steps (TDD, as the skill's shape):**
- [ ] **Step 1: Test the cue-and-exit behaviour first** with a scripted `Music`: (a) `musplus = -32` for ten ticks → the part draws nothing and stays `Continuing`; (b) `musplus = -11` → the fade runs 65 ticks then `Finished`; (c) `exit_requested` while still waiting → `Finished` on that tick.
- [ ] **Step 2: Run; fail.**
- [ ] **Step 3: Implement the part** (init, stages, `scr`, font, palette). Keep stage state in a `Phase` enum the test can read? Only if the test needs it; otherwise assert through the surface and the palette.
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison**: `cargo xtask compare --part mntscrl`-equivalent — the established procedure is to run the port headless over the reference keyframes 500, 1100, 1700, 2300 with support frames as D19 requires, then `cargo xtask compare` against `reference/parts/mntscrl`. Record exit code and percentages in `docs/fidelity/mntscrl.md`. Expect the fades and the scroll to be the risk; if a keyframe differs, D24's procedure decides.
- [ ] **Step 6: Write the note** in the same commit: what was translated, what was substituted, the cue's exact form, the preserved bugs, and the measured difference per keyframe with the command that reproduces it.
- [ ] **Step 7: Commit.**

**Acceptance:** the cue tests above; the four reference keyframes compare with a recorded verdict (exit 0, or a measured difference attributed under D24); `cargo test --workspace` green.

---

### Task 4: RAYSCRL — the raytraced mirror-ball scroll

**Files:**
- Create: `bins/secondreality/src/parts/rayscrl.rs`, `docs/fidelity/rayscrl.md`
- Modify: `bins/secondreality/src/parts.rs`, `bins/secondreality/src/shell.rs` (`music_for_part`), `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Consumes: `PosList` (Task 2), `sr_assets::load_path` (RIX3), `PartCtx`.
- Produces: `pub struct Rayscrl`, registered as `"rayscrl"`.

**The transliteration, from `WATER/DEMO.PAS`:**
- **Init** (`:89-116`): mode 13h; DIS start; busy-wait `musplus >= 0` and store the order (`co`); load `MIEKKA.SCI`'s palette and font (400×35 as the decoder delivers it; the part uses the first 400×34); black the DAC; blit `BKG.CLX`; fade tables.
- **Stage 1** (`:117-132`): 127-frame fade-in. **Stage 2** (`:135-143`): wait until the order **changes from `co`** and the row is **≥ 16**, with **no `waitb`** in the original — the port polls once per tick and consumes no CPU. **Stage 3** (`:155-189`): the main loop, no frame bound.
- **The cue**: `musplus == -11` exactly; 65-step fade then exit.
- **The blit**: `scr(sss)` writes one of the three WAT lists; font byte 0 replays the clean background pixel (colour-keyed, `ROUTINES.ASM:28-32`); every third tick the font shifts and 34 samples are inserted at `fbuf[158+x*158]`.
- **Bugs to preserve**: the fade loop `for pf := 0 to 3` touches `pal[x*3+3]` (the next colour's red) and reads `pal[768]` at x=255; the insert index `158+x*158` puts samples one band down; `move(fbuf[1], ...)` reads one byte past; `scp` stops at 390 with a 400-wide source.
- **Palette source**: `MIEKKA.SCI`'s palette wins over `BKG.CLX`'s (64 entries differ) — the reference is the check.

**Steps:** as Task 3 (cue-and-exit tests first; then the implementation; then the four keyframes 13000/13500/14000/14500; then the note; commit). The exit-while-waiting test is mandatory here because Stage 2 is the phase's only true spin.

**Acceptance:** as Task 3, against `reference/parts/rayscrl`.

---

### Task 5: DOTS — the mini vector balls (`minvball`)

**Files:**
- Create: `bins/secondreality/src/parts/minvball.rs`, `docs/fidelity/minvball.md`
- Modify: `bins/secondreality/src/parts.rs`, `bins/secondreality/src/shell.rs` (`music_for_part`), `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Consumes: `PartCtx`; no assets (tables are generated or transcribed from `SIN1024.INC`).
- Produces: `pub struct Minvball`, registered as `"minvball"`.

**The transliteration, from `DOTS/MAIN.C` and `DOTS/ASM.ASM`:**
- **Deterministic setup**: Turbo C's `rand()` is unseeded (seed 1); the 512 initial positions, the 500+500 shuffles and the stage-4 positions are all deterministic. Implement Turbo C's `rand()` exactly (its generator and modulus), and assert the first few values.
- **The stages** (`MAIN.C:196-282`): <500, <900, <1700, <2360 (with `grav--` every 32nd frame after 1900), <2400 (palette blue ramp), <2440 (fade to black), with the `dropper` and rotation rules.
- **The tick**: `repeat = dis_waitb()` in the original; the port's `frame()` advances the physics once and the stage positions `repeat` times — in the port `repeat` is the number of ticks the engine gave, which is one. **The original's `repeat` can exceed one** (the shell's copper may skip frames); the port's tick model is one per call, so the note must say so and the difference must be measured against the reference.
- **The drawing** (`ASM.ASM`): restore the old shadow pixel and three old ball rows from `bgpic`; project with `8<<16 / bp`; write the 2-pixel shadow at `[bx]` and the three ball rows at `bx+1` (word), `bx+320` (dword), `bx+641` (word); the ball rows come from `depthtable1/2/3`.
- **The cue**: in a demo run only (`dis_indemo`), `-4 < musplus < 0` breaks the loop. The port has no `dis_indemo`; the run is always "in demo", and standalone runs keep the same cue (the part's `standalone` flag decides nothing here — record the choice).
- **Bugs to preserve**: the off-screen branch's `mov ds:[si+6],ax` overwriting the stored shadow offset with a pixel value (`ASM.ASM:194-196`); the shadow colour 87 inside the background ramp.

**Steps:** TDD as Tasks 3/4 — cue tests (including the break on `-3..-1` and exit-while-waiting), the deterministic-setup test, the physics tests for one stage boundary, then the four keyframes 10900/11200/11800/12400 from `reference/parts/minvball/`, then the note, commit.

**Acceptance:** as Task 3, against `reference/parts/minvball`; plus the deterministic `rand()` test.

---

### Task 6: DDSTARS — the hidden part

**Files:**
- Create: `bins/secondreality/src/parts/ddstars.rs`, `docs/fidelity/ddstars.md`
- Modify: `bins/secondreality/src/parts.rs`, `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Consumes: `PartCtx`; `TEXTS.16` (Origin 4bpp, `sr_assets`' plane path); the EMS ring replaced by a plain `Vec<u8>` of 64×8K units (the original's INT 67h allocation is a memory strategy, not a picture).
- Produces: `pub struct Ddstars`, registered as `"ddstars"`.

**The transliteration, from `DDSTARS/STARS.ASM` and `KOE.ASM`:**
- **Mode**: 320×400, planar 16-colour; the port composes the two planes into the indexed surface the way the reference capture shows (the 7 non-identical DAC colours `do_stars` writes).
- **One tick**: flip the display page (`start_addr` 0x4000 ↔ 0); `waitb`; the 33-frame fade-in for `starframe<...`; the stage marks at 1200/1500/3200; `risetext` for the text wipe; advance and blit the stars; copy two EMS units per plane pair into the visible pages.
- **The stars**: 1024 entries of `(z, x, y)`; z is 8-bit through `movzx`; `z -= 2` and respawn from the LCG (seed 0, multiplier `0x343FD`, add `0x269EC3`, return DX) on underflow; projection `rot(x*9/8)/bp + 160`, `y*216/bp + 100`, `bp = 150 + 4z`; colour by z band (≥180 plane 0, 110–179 plane 1, <110 both).
- **The ring**: 64×8K units, units never cleared, so stars persist and recur every 64 frames — preserve.
- **D24**: the shipped `MAIN/DATA/DDSTARS.EXE` is a different (packed) build; the tree's text-clip tests are assembled out but the **reference shows text**. The port keeps the text visible and the note records which artifact each decision followed.
- **No music calls**: the shell cues MUSIC0@70; the part never reads it. The 320×400 reference and the sync band's reachability are Phase 3's A15; this part is why the band matters, and the note says so.

**Steps:** TDD — the LCG test (first outputs), the projection tests, the ring wrap test, then the four keyframes 1900/3000/5000/8000, then the note, commit. **The 8000 keyframe needs Task 1's `hiddenu2` trace for the cue-free comparison only** (DDSTARS has no cue; the trace is needed to place the part in demo time).

**Acceptance:** as Task 3, against `reference/parts/ddstars`; plus the LCG and ring tests.

---

### Task 7: ALKU — the intro text and the horizon pan

**The phase's hardest part: the first ported part to use the copper, and the first to pan the view.**

**Files:**
- Create: `bins/secondreality/src/parts/alku.rs`, `docs/fidelity/alku.md`
- Modify: `bins/secondreality/src/parts.rs`, `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Consumes: `PartCtx::set_copper` (three slots), `View` (`start_addr`, `x_offset`, `pan`, `double_scan`, `y_line_offset`), `Font::from_image`, `sr_assets::load_path`.
- Produces: `pub struct Alku`, registered as `"alku"`.

**The transliteration, from `ALKU/MAIN.C`, `ASMYT.ASM`, `COPPER.ASM`:**
- **The display**: 320×400 output from a 640×200 picture; 176-byte source rows, each carrying two 352-pixel copies at `+88`, the CRTC start and pixel pan selecting the visible 320-pixel window. In the port: a 640×200 indexed source surface, and a `View` whose `double_scan` gives 400 lines and whose `start_addr`/`x_offset`/`pan` reproduce `cop_start = a/4 + p*88`, `cop_scrl = (a&3)*2`.
- **The copper**: routine 1 before retrace writes the CRTC start/pan; routine 2 at retrace increments `frame_count` and uploads the palette when `do_pal`; routine 0 after the display start applies the 8.8 fade into `fadepal`. The engine's slot order is 0 → bands → 1 → 2; the mapping of the original's three registrations (ax=1,2,0 → routines 1,2,3) onto the port's slots is a decision the task records.
- **The stages**: card 1 after `dis_sync()<1`, card 2 after `<2`, card 3 after `<3`, the picture after `<4`; the text batches wait `dis_sync() < 4+tptr` **with `a&1`** (`MAIN.C:138`), and `tptr` is post-incremented so batch 0 waits for code 5, not 4 — preserve the off-by-one and say so.
- **The scroll**: `do_scroll` waits until `frame_count >= 9` and subtracts 9; the port's tick is one frame, so the scroll advances every 9 ticks with a counter — the original's `frame_count` is the copper's frame counter, which the port's engine does not expose; the task must decide whether the port's tick *is* the copper frame (it is — one tick per frame) and state it.
- **The assets**: `hzpic` = the linked `HOI.IN0+IN1` bytes, which differ from `HOI.U` in 3 palette entries (D24/E.18) — the port reads `HOI.U` (the curated asset) and records the three entries; `font` = `FONA.UH[783:]`, shifted one byte (A9), and `Font` refuses a 30-row strip against the 32 rows the routine reads — A9's resolution stands.
- **Bugs to preserve**: the palette write-order aliasing that repeats entries 192–767; the source column reaching 663 > 640 at `a=319`; the `memset` clearing `186*320` of a `186*352` buffer; the `while(frame_count<9);` spin (port: ticks).

**Steps:** TDD — the `dis_sync` stage gates (synthetic `Music` returning codes 0..5, including a skipped code), the scroll's 9-tick cadence, the pan arithmetic (`cop_start`/`cop_scrl` → `start_addr`/`x_offset`), the font shift, then the four keyframes 900/1719/2719/5859, then the note, commit. **The 5859 keyframe is moving** (1–2 px pan) and D19's support-frame rule applies.

**Acceptance:** as Task 3, against `reference/parts/alku`; the copper slots exercised in a test; the cue gates pinned.

---

### Task 8: PAM — the Praxis explosion

**Files:**
- Create: `bins/secondreality/src/parts/pam.rs`, `docs/fidelity/pam.md`
- Modify: `bins/secondreality/src/parts.rs`, `bins/secondreality/tests/shell.rs`

**Interfaces:**
- Consumes: `sr_assets::load_frames` (ANI), `pal::decode`, `PartCtx`.
- Produces: `pub struct Pam`, registered as `"pam"`.

**The transliteration, from `PAM/OUTTAA.C` and `ASMYT.ASM`:**
- **The wait**: `dis_sync() < 10` (`:39`) before the first frame.
- **The animation**: 42 delta frames in `OUT.ANI`; each decoded frame is a delta from frame *n−2* (the packer emulated the two-page flip); the port reproduces the two-page flip through `View::set_origin` (0 and 0x4000) and decodes into the off-screen page.
- **The loop**: 45 steps, `frame_count >= 4` between steps (four ticks per animation frame); only 40 frames are drawn; the first uses `wfade[1] = 32` (half wash), not 63; `wfade` has 59 values but its used range is 1–45.
- **The palette**: entry 63 near-white; the fade table indexes 64 palette copies.
- **Bugs to preserve**: the off-by-one between the FLI header's 41 frames and the container's 42; `ruudulle` dead; the `while(frame_count<4);` spin (port: ticks).
- **A3/A4** already register the decoder's malformed-input behaviour and two format choices — the note cites them rather than re-deriving.

**Steps:** TDD — the `dis_sync()<10` gate, the four-tick cadence, the 40-of-42 frames and `wfade[1]`, the page flip; then the four keyframes 7710/7760/7830/7900, then the note, commit.

**Acceptance:** as Task 3, against `reference/parts/pam`.

---

### Task 9: The shell — the six parts in their blocks

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (`BLOCKS`, `block_plan`, `music_for_part`, `boot_anchor`), `bins/secondreality/tests/shell.rs`, `docs/fidelity/part-sequence.md` if a block's contents change.

**Interfaces:**
- Consumes: the six registry names (Tasks 3–8).
- Produces: `--block 1` plays ALKU then stops at U2A (unported); `--block 3` plays the four ported block-3 parts in the demo's order until the first unported one; `--block U` plays DDSTARS alone; `--part <name>` works for all six.

**Steps:**
- [ ] **Step 1: The block plans.** `BLOCKS` already names each part; `block_plan` filters by `PART_ORDER`. Change the plan so a block stops at the first unported part (the established rule) and the status line says why. Tests: `--block 1` starts ALKU; `--block 3` starts at the first ported block-3 part; `--block U` starts DDSTARS.
- [ ] **Step 2: `music_for_part`.** `rayscrl` and `minvball` are mid-block and need rows (their module/order at the point the demo reaches them); `mntscrl` and `ddstars` are block firsts and resolve through `music_for`. Derive each from `U2.ASM:856-874` and the trace (Task 1), record it as measured or derived, and extend the test that asserts every registry name has a row.
- [ ] **Step 3: Anchors.** A part with a measured reference anchor gets one (`boot_anchor`); otherwise its frame numbering is part-relative and the status line says so (the established behaviour). State per part.
- [ ] **Step 4: The end-to-end run.** `--block 1` and `--block 3` and `--block U` headless, asserting the parts start in the demo's order and the music rows match.
- [ ] **Step 5: Commit.**

**Acceptance:** the shell tests; a headless run of each block reaching its ported parts in order.

---

### Task 10: The phase's verification and close-out

**Files:**
- Create/modify: `PROGRESS.md`, `docs/decisions.md`, `docs/fidelity/APPROXIMATIONS.md`, the six notes, `docs/fidelity/part-sequence.md`.

**Steps:**
- [ ] **Step 1: Re-run every criterion at the committed state**, recording the numbers verbatim: each part's four keyframes under its own invocation; the workspace tests; both clippy commands; `reference verify`; `assets verify --decode`.
- [ ] **Step 2: The falsification list** in Phase 2/3's shape: each constant or rule (the cue forms `-11`, `-4<·<0`, `<N`; the 9-tick scroll; the 4-tick cadence; the stage bounds; the pan arithmetic; the LCG), its derivation, what a one-unit error would change, and the strongest independent check — naming honestly which have none.
- [ ] **Step 3: The negatives**, at least: which parts' cues rest on new traces and which on synthetic sequences; the `repeat`-vs-tick difference in DOTS; DDSTARS's D24 divergence; A9's font shift; any part whose keyframes are RGB-only and why; the same-format caveats.
- [ ] **Step 4: The record**: `PROGRESS.md`'s phase row (complete, untagged), the evidence section, the "what Phase 4 leaves Phase 5" list; `decisions.md` for anything the phase decided; `APPROXIMATIONS.md` — **and A9's "Phase 5" label, which the reference survey found inconsistent with spec line 701, corrected to Phase 4**.
- [ ] **Step 5: Nothing is tagged here.** The final whole-branch review runs first; the tag goes on after its findings are applied (Phase 1/2/3's practice).

**Acceptance:** every number reproduced by the command the record names; the six notes complete; the suite green.

---

## Self-review notes (for the plan itself)

- **Coverage**: spec §10's Phase 4 exit criterion names exactly these six parts; each has a task; the shell and close-out are Tasks 9–10; the traces Task 1; the shared decoder Task 2.
- **Order**: Task 1 gates the cue evidence for Tasks 3–8; Task 2 gates Tasks 3, 4; the parts are independent of each other apart from the registry; Task 9 needs all six; Task 10 needs everything.
- **Known risks the plan carries open**: ALKU's copper-slot mapping (routine 1/2/0 → slots) is a decision each implementer must make and record, because the engine's slot order is fixed and the original's is not the same thing; DOTS's `repeat` vs the port's one-tick model; DDSTARS's D24 divergence; and whether a part's frame numbering is anchored or part-relative (Task 9 Step 3).
