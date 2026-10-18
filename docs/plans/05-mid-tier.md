# Phase 5 — Mid tier (PLZPART, TECHNO, LENS, PANIC, JPLOGO, COMAN) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the six mid-tier parts — PLZPART (plasma + cube), TECHNO (interference + bars + troll), LENS (`lnszoom`, lens + rotazoomer), PANIC (`panicend`, the fake ending), COMAN (`3dsinfld`, voxel terrain) and JPLOGO (the jelly logo) — so each runs against the reference's own keyframes, each waits on the music exactly as the original does, and block 3's chain runs end to end for the first time.

**Architecture:** Six part modules under `bins/secondreality/src/parts/`, each a transliteration of its upstream source into the existing `Part` API. Phase 5 also lands the seam Phase 4 deferred: a `PartCtx` flag saying whether the run's player was just installed for this part (an entry-point run) or was already running (a mid-chain start), so the entry pre-rolls and PAM's open-gate counter are skipped in a chain. PLZPART's data tables are the original's `.INC` text fragments, filed under `assets/PLZPART/` and parsed in their original format. The acceptance is each part's per-part integration test against `reference/parts/<part>/`, plus a block-3 chain test with the real player.

**Tech Stack:** Rust 2021, `crates/sr-engine` (part API, surface/view, copper, palette), `crates/sr-assets` (Origin/IFF/RIX3/ANI/`.INC` readers), `xtask` (reference capture/compare, music trace), SDL2 only in the binary.

**Spec:** `docs/spec/2026-10-10-second-reality-rust-design.md` §2.2 (part sequence) and §10 Phase 5's exit criterion: "PLZPART, TECHNO, LENS, PANIC, JPLOGO, COMAN verified". Binding rules: `docs/decisions.md` (D4, D7, D8, D15–D17, D19, D21, D22, D24, D26–D30), `docs/fidelity/APPROXIMATIONS.md` (A1, A3, A4, A9, A12, A15–A21) and `CLAUDE.md`.

## Global Constraints

- **The reference is the authority; the object and the shipped release are the tiebreakers.** The vendored tree is a development snapshot (D24); where a reference frame disagrees with a faithful tree port, D24's procedure decides and the divergence is recorded.
- **Port the algorithm, not a resemblance.** Fixed-point arithmetic, table values and rounding are transliterated, including the originals' *bugs* (D7). Every approximation is registered in `docs/fidelity/APPROXIMATIONS.md` with its measured difference. The palette upload truncations (`_setpalarea`'s 512-byte write in TECHNO/JPLOGO, PLZPART's leak, PANIC's rotated `getpal`) are output, not defects to clean.
- **Assets are read in their original formats.** No conversion. `reference/` and `upstream/` are input-only; nothing is written into them. PLZPART's `.INC` fragments are filed under `assets/PLZPART/` verbatim, with provenance, because they are the part's data.
- **6-bit palette internally**, widened only at upload (`dac_expand(v) = (v*259+33)>>6`, D16).
- **Never spin-wait.** The logic tick is fixed at 70.086 Hz; a part that waits for a cue returns `Continuing`. Every original `while(...)` is a tick-consuming wait. A part whose source never polls `dis_exit` (PANICEND) is retired by the shell at the end of the tick the flag was set in; its own waits still must not loop.
- **The view's semantics are D16's**: `start_addr` is a pixel address, `x_offset` is within a line, a split's lower view reads origin 0, `y_line_offset` defaults to the surface stride. CRTC plane-byte starts convert to pixel origins ×4.
- **`sr-engine` keeps zero runtime dependencies; `sr-audio` keeps zero; `sr-assets` may depend on `sr-engine` only.**
- **Every number in a note is reproducible by a command the note names, or is produced by a test in the same commit** (D25). A quoted block is a paste or a marked elision (D29). Cite the upstream source's actual lines; several `reference-indices.md` citations predate the pinned files and are off by a few lines.
- **Part names are the demo's binaries**: `plzpart`, `lnszoom` (LENS), `techno`, `panicend` (PANIC), `3dsinfld` (source dir `COMAN`) and `jplogo`. `3dsinfld` is not a legal Rust identifier: the module is `parts/coman.rs` and the test target `tests/coman.rs`, while the registry/plan name stays `3dsinfld`.
- **The mid-chain rule is binding**: a part's entry pre-roll and PAM's open-gate counter run only when the run's player was just installed for this part; a part reached mid-chain must not poll the already-running player (Phase 4's final-review Important 1).
- **Tests and clippy are the gate**: `cargo test --workspace` green, zero warnings, both clippy commands clean. Commits end with `Co-Authored-By: Claude Code <noreply@anthropic.com>`.

## Review Focus

Five failure modes the spec implies but no single task's tests would otherwise catch. Each is pinned by a named test in the task that owns the code.

1. **A mid-chain start must not advance the already-running player.** MINVBALL/RAYSCRL/MNTSCRL pre-poll 256/268/11 in `init`, and PAM sets `frame_count = 2`, all calibrated for entry-point runs. The first runnable block-3 chain reaches them after LNS&ZOOM/PLZPART with the player continuous; the seam task must guard every one, and the block-3 chain test must assert the real player's positions (`75:61` at MINVBALL, `88:0` at RAYSCRL) in the chain.
2. **A part with two screens or three acts must transition the view exactly.** PLZPART's plasma (320×400, split 61/62, 96-byte rows, `cop_start`) → cube (320×200, 160-byte rows, six-page flip); TECHNO's 320×200 EGA/16-colour → 320×400 tweak (160-byte rows, page flip); LENS's face → lens → rotazoomer (mode tweak, 400-line planar, DAC clear). A missed register or stride shows the wrong picture at the next keyframe; each part's note names the transition and a test exercises it.
3. **The original's palette bugs are the picture.** TECHNO's and JPLOGO's `_setpalarea` writes `2*count + start` bytes (512 of 768), PLZPART's `memset(fadepal,0,768)` leaves the fractional halves (the leak/banding), PANIC's `getpal` starts the DAC read at index 13 and its fade tables leave entry 0 black. A "fixed" upload fails the diff; the tests pin the byte counts and the surviving entries.
4. **PANICEND inherits its predecessor's screen and DAC.** Its collapse reads the previous part's VGA picture (`getpal`, `kuva`), so a standalone `--part panicend` run starts on a blank surface and cannot match 8792/8800. The evidence is a handoff test that drives TECHNO's port to its end state and then PANICEND on the same surface/palette, plus the self-contained 9000 comparison; the note says which keyframe is which kind of evidence.
5. **A source that never polls `dis_exit` must still be interruptible.** PANICEND has no `dis_exit` call and no exit condition of its own; its 60-frame pulse runs to the end. The port's part must not spin, and the shell's retire-on-exit path must be exercised (ESC during the collapse and during the pulse), with the note recording the shell-owned retirement.

---

## The measured ground truth

Every row is from the source survey; **the implementer re-derives each against the file**, because the plan is an argument and the tree is the evidence.

| Part | Source | Stages / exit | Music calls | Assets | Reference keyframes | Start / block |
|---|---|---|---|---|---|---|
| PLZPART | `PLZPART/{PLZ.C 219, PLZFILL.C 186, VECT.C 299, PLZA.ASM 375, COPPER.ASM 300, SPLINE.ASM 88, ASMYT.ASM 119}` | plasma: wait `musplus<0`, `setmframe(0)`, timetable 723/1491/1875/2259 (via `getmframe`), exit `curpal==5 && cop_drop>64`; cube: wait `musplus<13`, exit `-4<=musplus<0`; split 61/62, pan 4/0, `do_drop` | `musplus` (`PLZ.C:68`, `VECT.C:116,119`), `setmframe` (69), `getmframe` (79); no sync/row | none as media; `.INC` tables (`PSINI`, `LSINI4`, `LSINI16`, `SINIT`, `PTAU`, `SPLINE`, `RATA`) + textured cube `kuva1..3`, `dist1` | plasma 6000/7000/7600 (RGB only), cube 8500/9500 | (5720,5730] / block 3 |
| LENS (`lnszoom`) | `LENS/{MAIN.C 394, ASM.ASM 566, CALC.C 172}` | part1 wait `musplus<-6` then `setmframe(0)`, 300 frames; part2 715 frames; gate `musplus<-20`; part3 break `musplus>-4`, 2000 frames; white exit | `musplus` (`MAIN.C:104,213,380`), `setmframe(0)` (106); no sync/row | `LENS.EX0..4`, `LENS.EXB`, `LENS.EXP`, `FILLER` | 2700 (idx), 3400/4200/4900, 5650 (idx) | (2452,2456] / block 3 |
| TECHNO | `TECHNO/{KOE.C 728, KOEA.ASM 1065, KOEB.ASM 632, POLYCLIP.ASM 273, READP.C 72}` | wait `musplus<-4`, `setmframe(0)`; 256-frame interference; `musplus<-3`; four row-`&7==7` wipes; `getmframe>2520`; doit1/2/3 420/840/980 frames with order>35/row>48 and `musplus in (-6,16)` | `musplus` (`KOE.C:213,275,708`), `setmframe(0)` (214), `musrow` (39,279,305), `getmframe` (495) | `CIRCLE2.EGA`, `TROLL.UP`; linked `_CIRCLE` = `assets/DDSTARS/PIC.EGA` | interference 4900/5000/6300/8100 (RGB), troll 8200(.idx all 0)/8300/8400 | 4797 / block 2 |
| PANIC (`panicend`) | `PANIC/SHUTDOWN.C 157, TWEAK.ASM 325` | no music calls, no `dis_exit` poll; `getpal`; 64 fade palettes (`b=3..`); collapse 32→3 by `a*5/6`; line + dot; 60-frame pulse; `sleep(1)` | none | `MONSTER.U`/`MONSTER.PAL` are compiled out; the live path is TECHNO's screen + DAC | 8792/8800 (RGB), 9000 (idx) | (8780,8790] / block 2 |
| COMAN (`3dsinfld`) | `COMAN/{MAIN.C 228, ASM.ASM 238, THELOOP.INC 5230, THELOOP.ASM 11}` | wait `musplus<0`; loop `frame<4444`; break `-8<musplus<0`; `frepeat=waitb`; two pages (0x0c 0x80/0x00); `docopy` 160→320 | `musplus` (`MAIN.C:56,59`) | `W1DTA.BIN`, `W2DTA.BIN` (linked; `COMBG` is not in the link) | 15500/16000/16500/16800 (RGB) | (15000,15100] / block 3 |
| JPLOGO | `JPLOGO/{JP.C 227, ASM.ASM 156, READP.C 64, ZOOM.INC 16837}` | wait `musplus<4`; loop `frame<700`; freeze after 511; `scrolly` start 400→0; spring tables + row scale; dirty-row redraw | `musplus` (`JP.C:71`) | `ICEKNGDM.UP` (linked `_PIC.OBK`) | 17300/17500/17700 (RGB) | (17000,17100] / block 3 |

---

### Task 1: The three music traces Phase 5 needs

**Files:**
- Create: `reference/music/second2/` (frames 4700–9300: TECHNO 4797–8780 and PANICEND 8780–~9230), `reference/music/second3a/` (frames 2900–10200: LENS 2900–5725, PLZPART 5726–10399), `reference/music/second3d/` (frames 15000–18000: COMAN 15100–16900, JPLOGO 17000–18000)
- Modify: `docs/fidelity/music-clock.md` (§2's trace table and coverage), `xtask/tests/verify.rs` and `xtask/tests/music.rs` (explicit lists/floors), `crates/sr-audio/tests/trace.rs` if its set is explicit.

**Interfaces:**
- Consumes: `cargo xtask music trace` (cycles 30000, the boot/keys defaults), the trace format and manifest fields the eight existing traces use.
- Produces: three trace directories with manifests; `music-clock.md` naming all eleven traces and their windows; the measured order in force at each Phase 5 part's start (the `music_for_part` rows Task 10 consumes).

**Why these three windows:** Phase 4 filed `second3` (144–2899) and `second3b` (10200–15049) but deliberately left 2900–10200 (LENS/PLZPART) and 15050+ (COMAN/JPLOGO) uncaptured, and there is no `SECOND 2` trace at all (TECHNO/PANICEND). The windows are: `second2` 4700..9300 (both block-2 parts and the PANICEND→MNTSCRL boundary), `second3a` 2900..10200 (joins `second3` to `second3b`), `second3d` 15000..18000 (COMAN + JPLOGO, with the 15000-overlap that puts COMAN's start window in one session). Do not widen them; `--frames A..B` excludes B.

- [ ] **Step 1: Capture `second2`**: `cargo xtask music trace --arg 2 --cycles 30000 --frames 4700..9300 --out reference/music/second2`.
- [ ] **Step 2: Capture `second3a`**: `--arg 3 --cycles 30000 --frames 2900..10200 --out reference/music/second3a`.
- [ ] **Step 3: Capture `second3d`**: `--arg 3 --cycles 30000 --frames 15000..18000 --out reference/music/second3d`.
- [ ] **Step 4: Verify and extend the sets.** `cargo xtask reference verify --tree reference` passes; `cargo test -p sr-audio --test trace` covers eleven traces (extend an explicit list/floor if the test has one); `cargo test -p xtask --test music` and `--test verify` pass.
- [ ] **Step 5: Record the windows and the per-part orders** in `music-clock.md`: each trace's name, frames, module, anchor/spread; and, for each of the six parts, the order in force at its start window (LNS is in `second3` at 2452: MUSIC1 order 45; the other five come from these traces). These values are Task 10's inputs.
- [ ] **Step 6: Commit.**

**Acceptance:** `reference verify` passes; the trace test covers eleven; the note names every trace and the six measured orders.

---

### Task 2: The carried Phase-4 fixes

**Files:**
- Modify: `bins/secondreality/src/parts/{mntscrl,rayscrl}.rs` (bound the position-list offsets in `init`), `docs/fidelity/alku.md`, `docs/fidelity/minvball.md`, `docs/fidelity/APPROXIMATIONS.md`
- Test: the two parts' unit tests (malformed-list cases).

**Interfaces:**
- Consumes: `PosList` (Task 2 of Phase 4); the parked findings in `.superpowers/sdd/04-music-synced-tier-a/progress.md`.
- Produces: `PartError` on a list offset that would leave the 64,000-byte background, tested with a hand-built list; the parked prose corrected.

**Steps:**
- [ ] **Step 1: Add the offset bound.** In both parts' `init`, after parsing, assert every column offset is `< 64_000` (the background length); return the part's usual `PartError` naming the file, column and offset otherwise. Test: a synthetic list with an offset of 65,000 returns `Err`, not a panic.
- [ ] **Step 2: Fix the parked prose.** `alku.md`'s "do-while" → `for` (`ALKU/MAIN.C:87`); `minvball.md`'s mapping "pins" wording aligned with the regraded falsification row (the 12,900 `c=59` observation is the independent pin; the keyframes are the fit's own evidence); A20's `the_5859_phase_sensitivity` citation notes it runs with `--ignored`.
- [ ] **Step 3: Run the two parts' tests and commit.**

**Acceptance:** both malformed-list tests pass and would fail without the bound; no note contradicts its part's record.

---

### Task 3: The mid-chain music seam

**Files:**
- Modify: `crates/sr-engine/src/part.rs` (`PartCtx` gains the flag; `PartCtx::new` gains the argument), `crates/sr-engine/src/engine.rs` (set on `set_music`, consumed by the first `start`, read by `PartCtx`), `bins/secondreality/src/shell.rs` (`install_music` marks it), `bins/secondreality/src/parts/{minvball,rayscrl,mntscrl,pam}.rs` (guard their pre-rolls/counter on it)
- Test: `crates/sr-engine/src/engine.rs` unit tests, the four parts' pre-roll tests, a new chain test in `bins/secondreality/tests/shell.rs` (with Task 10).

**Interfaces:**
- Produces: `PartCtx::music_fresh(&self) -> bool` — true only for the first part started after the run's music was installed (the entry-point run); false for every later part in the chain. `PartCtx::new` gains a `music_fresh: bool` argument (19 call sites, all mechanical).
- The four parts consume it: MINVBALL's 256, RAYSCRL's 268, MNTSCRL's 11 and PAM's `FRAME_COUNT_AT_ANCHOR = 2` apply only when `music_fresh`; otherwise zero polls and PAM's post-gate setup with `frame_count = 0`.

**Steps:**
- [ ] **Step 1: Write the failing engine test.** `the_music_is_fresh_only_for_the_first_part_after_set_music` — a sequence of two stub parts sees `true` then `false`; a second `set_music` makes the next part fresh again.
- [ ] **Step 2: Run it; fail.**
- [ ] **Step 3: Implement the flag** through `PartCtx::new`, `Engine::set_music`/`start`, and the four part guards.
- [ ] **Step 4: Update the four parts' tests**: each pre-roll test gains a mid-chain case asserting zero polls (and PAM's open gate with `music_fresh=false` sets `frame_count = 0`).
- [ ] **Step 5: Run the workspace suite and commit.**

**Acceptance:** the engine test and the four parts' mid-chain cases pass; every existing entry-point pre-roll test still passes unchanged.

---

### Task 4: PLZPART — the plasma and the cube

**Files:**
- Create: `bins/secondreality/src/parts/plzpart.rs`, `bins/secondreality/tests/plzpart.rs`, `docs/fidelity/plzpart.md`, `assets/PLZPART/{PSINI.INC,LSINI4.INC,LSINI16.INC,SINIT.INC,PTAU.INC,SPLINE.INC,RATA.INC}` + provenance; modify `parts.rs`, `docs/fidelity/asset-inventory.md`, `docs/fidelity/APPROXIMATIONS.md` if a divergence is accepted.

**Interfaces:**
- Consumes: `PartCtx` (`set_palette`, `set_copper`, `surface`, `view`, `music`, `music_fresh`), `sr_assets`.
- Produces: `pub struct Plzpart`, registered as `"plzpart"`; a small `.INC` reader for `db`/`dw` text (part-local) that produces the tables in their original byte/word order.

**The transliteration, from the survey (re-derive each line):**
- **Data:** the `.INC` fragments are the part's tables; copy them verbatim into `assets/PLZPART/` and parse `db`/`dw` text. The hard-coded assembler offsets `0xC100/0xC200/0xC300/0xC400` (`PLZA.ASM:46-61`) are the table base displacements; verify them against the map (`PLZPART.MAP:368-369,391`) and the shipped binary.
- **Plasma:** `plz()` `PLZ.C:62-122`: wait `musplus<0`, `setmframe(0)`, `init_plz`, the five `pals[6][768]` tables scaled by `(v-63)*2` and `*8`, the timetable {723,1491,1875,2259,0} read through `getmframe`, two plane passes (masks `0x0a02`, `0x0502`), four row loops (`y+=2`), exit `curpal==5 && cop_drop>64`; `moveplz`'s parameter walk (`COPPER.ASM:145-173`), `do_drop`'s split walk (`:175-268`) and the 16-bit `fadepal` accumulators. Display: 320×400, 96-byte plane rows, `cop_start = 96*282`, pan 4/0, split 60/61 (register+1).
- **Cube:** `vect()` `VECT.C:110-130`: `tw_opengraph`, wait `musplus<13`, exit `-4<=musplus<0`; `calculate(1)/draw(15)/clear()`; the six-page `swappage` (`VECT.C:265-298`), camera spline `getspl(4*256+frames*4)`, rotation constants and `xxx=(xx*256)/zz+160+160`, `yyy=(yy*142)/zz+66`; the span rasteriser and textured cube in `PLZA.ASM:59-259`; `do_clear`'s edge lists.
- **Copper mapping:** `init_copper` registers `copper1` on slot 1, `copper2` on slot 2, `copper3` (empty) on slot 0; the engine's order 0 → bands → 1 → 2 differs from the original's slot names — decide and record the mapping by what each routine does.
- **Bugs to preserve:** the 84-byte `plzline` overrun into the 96-byte stride gap; `do_clear`'s zero-count mask underflow; the palette-leak banding (`memset` of 768 of 1,536 bytes); the cut-short fifth fade; the dead `ttptr` row; the 404 split value; the stale `MAIN.ASM` is not part of the port.
- **Reference:** plasma 6000/7000/7600 and cube 8500/9500 under `reference/parts/plzpart/{plasma,cube}/` (RGB only); all five moving, the plasma's spread sub-frame; the start is a window (5720,5730], so numbering is part-relative with a derived mapping (D30).

**Steps:**
- [ ] **Step 1: Write the failing tests** — the `.INC` reader's exact sizes and byte/word order; the plasma timetable's events at the `getmframe` thresholds; the split/pan arithmetic; the cube's projection constants; `do_drop`'s first and last steps.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (data reader, plasma, cube, copper mapping).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison** — the five keyframes across the two reference directories at the derived mapping.
- [ ] **Step 6: Write the note** in the same commit, with the copper mapping, both screens and the preserved bugs.
- [ ] **Step 7: Commit.**

**Acceptance:** the five keyframes compare with a recorded verdict (exit 0, or a D24-attributed difference measured); the presentation-list bugs pinned; the note covers both screens and the copper mapping.

---

### Task 5: LENS — the lens and the rotazoomer

**Files:**
- Create: `bins/secondreality/src/parts/lnszoom.rs`, `bins/secondreality/tests/lnszoom.rs`, `docs/fidelity/lnszoom.md`; modify `parts.rs`, `shell.rs` (music row), the `expected` table.

**Interfaces:**
- Consumes: `PartCtx`, `sr_assets` (`load_path`, the `.EX*` blobs and `LENS.EXB`'s raw picture).
- Produces: `pub struct Lnszoom`, registered as `"lnszoom"`.

**The transliteration (from the survey):**
- **Init** (`MAIN.C:278-372`): the palette at `LENS.EXB+16`, the picture at +784, the 1,536-byte overrun at `back+64000`; `LENS.EXP`'s `pathdata1` (715 frames ×2) and `pathdata2` (2000 ×4); the 64+16 fade palettes and 32 `fade2` blocks; `rotpic[256][256]` sampling `y*10/11-18` with row 0 clamp.
- **Part 1** (`:89-131`): the `musplus<-6` wait (the source's `if(dis_musplus>-30)` missing-call bug is preserved), `setmframe(0)`, 300 frames, the two wipe columns for frames <80, `frame += waitb()`.
- **Part 2** (`:133-183`): 715 frames, `fade2` uploads for uframe<96, the `pathdata1` lens at `(x,y)`, the four table passes (`dorow`, `dorow2`, `dorow3`) and the `+`1,536/+64,000` guards; the 5-frame hold.
- **Part 3** (`:185-276`): the 90° rotation in place, the `inittwk` tweak (400-line planar, DAC clear), the `musplus>-4` break, `pathdata2`'s `rotate`, the frame-1872 fade, the first-16 white steps, the 768×63 white exit.
- **Bugs to preserve:** the missing `()` in `if(dis_musplus>-30)`; the `setpalarea` call at `:200` erased by `inittwk`; the 1,536-byte overrun; `rotpic90=back`; the `a>3` path clamp; `rotatez` dead.
- **Reference:** 2700 (idx), 3400, 4200, 4900, 5650 (idx); 2700/5650 held; start window (2452,2456]; part-relative mapping.

**Steps:**
- [ ] **Step 1: Write the failing tests** — the three acts' lengths and gates; the lens table's bounds and the 1,536-byte overrun; the rotazoomer's aspect correction and 90° rotation.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (init, the three acts, the exit).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison** — 2700/3400/4200/4900/5650 at the derived mapping.
- [ ] **Step 6: Write the note** in the same commit.
- [ ] **Step 7: Commit.**

**Acceptance:** the five keyframes; the gates and the broken `if` pinned; the note covers the three acts and the white exit.

---

### Task 6: TECHNO — the interference and the troll

**Files:**
- Create: `bins/secondreality/src/parts/techno.rs`, `bins/secondreality/tests/techno.rs`, `docs/fidelity/techno.md`; modify `parts.rs`, `shell.rs` (music row), the `expected` table.

**Interfaces:**
- Consumes: `PartCtx`, `sr_assets` (`CIRCLE2.EGA` raw, `TROLL.UP` via the Origin reader, `assets/DDSTARS/PIC.EGA` as `_circle`).
- Produces: `pub struct Techno`, registered as `"techno"`.

**The transliteration (from the survey):**
- **Init** (`KOE.C:109-211`): the bar palette, `power0/power1`, `asminit`, the `vbuf` 8,192-byte mask.
- **Stage 1** (`:213-216`): `musplus<-4` wait, `setmframe(0)`, `dointerference2`'s 256-frame palette fade (the 5000 white).
- **Stage 2** (`:218-316`): `initinterference`, `dointerference` (exit at music frame 925), the five flashes, the 16-colour screen (CRTC `13h=20`, ATC `13h=0`, map mask `0x0f`), the four bar wipes with row waits.
- **Stage 3** (`:317-338`): the second `dis_partstart` (preserve), `doit1/2/3` with `waitborder`, the `getmframe>2520` shrink, the order>35/row>48 wait, the 300-frame pan, the 50-frame ripple, the final `musplus in (-6,16)`.
- **Troll** (`:605-633`): the 80,000-byte far buffer and the segment poke, `readp`, `lineblit` at 160-byte rows, the 16 fade palettes.
- **Bugs to preserve:** `_setpalarea`'s 512-byte upload; `setborder` dead; `flash(-2)`; the 400 overwritten `cx`; the dead KOEB plane-8 loop and `sizefade`; `patdir` only ≤0; `rotate1`'s last 64 bytes; the unlinked `_COMBG` analogue `_PIC1/2` are not linked.
- **Display:** two geometries; page flips 8-page/4-page; `start_addr`/`x_offset` mapping for the 160-byte rows; no copper.
- **Reference:** interference 4900/5000/6300/8100 (RGB), troll 8200 (idx all 0)/8300/8400; start 4797 (a content-probed frame; sessions within 1–3 frames), part-relative.

**Steps:**
- [ ] **Step 1: Write the failing tests** — the bar quad arithmetic; the 512-byte palette upload; the interference's 256-frame fade; the two geometry transitions; the troll's RLE and row duplication.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (the three stages, both screens).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison** — the seven keyframes across `reference/parts/techno/{interference,troll}/`.
- [ ] **Step 6: Write the note** in the same commit.
- [ ] **Step 7: Commit.**

**Acceptance:** the seven keyframes; the palette truncation and the dead paths pinned; the note covers both screens and the gates.

---

### Task 7: PANICEND — the collapse

**Files:**
- Create: `bins/secondreality/src/parts/panicend.rs`, `bins/secondreality/tests/panicend.rs`, `docs/fidelity/panicend.md`; modify `parts.rs`, `shell.rs` (music row), the `expected` table.

**Interfaces:**
- Consumes: `PartCtx` (the live surface from the predecessor, the live palette), `sr_assets` (not on the demo path).
- Produces: `pub struct Panicend`, registered as `"panicend"`; the part polls no music and no `dis_exit`; `frame()` returns `Continuing` until the pulse ends.

**The transliteration (from the survey):**
- **Init** (`SHUTDOWN.C:30-55`): `dis_partstart`; the `_LOADER_` branch is the live path — `getpal` reads the DAC starting at index 13 (rotated), no `MONSTER.U/PAL`.
- **Collapse** (`:57-137`): row-0 blacking; the 64 fade palettes (`b=3..767`); the first 100-row copy; `tw_setstart(16000)`; the inline CRTC `09h=41h`, GC `05h=41h`, map mask; the 32→3 `a*5/6` loop with write-mode-1 latch copies (80 words = 160 bytes); the offset `13h=0Ah`; the line's symmetric pixel writes and the colour-1 dot; the 60-frame `cos` pulse; `sleep(1)` → retire.
- **Bugs to preserve:** `getpal` at index 13; entry 0 black in the fades; the word-count `copyline`; the integer collapse ending at 3; no clear beyond row 0; no exit poll.
- **Evidence:** 8792 (moving, the shrinking band) and 8800 (held, the line+dot) depend on the predecessor's screen/DAC; 9000 (held, the dot) is self-contained (one index-1 pixel at (160,100)). The handoff test drives TECHNO's port to its end state and PANICEND on the same surface/palette to compare 8792/8800; the standalone integration test compares 9000 and asserts the rest is the recorded limitation.

**Steps:**
- [ ] **Step 1: Write the failing tests** — `getpal`'s rotation by 13; the fade tables' entry-0 black; the collapse's latch-copy bounds; the line/dot coordinates; exit during the collapse and during the pulse.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (the collapse, the line/dot, the pulse).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The comparisons** — the handoff test for 8792/8800, the standalone test for 9000.
- [ ] **Step 6: Write the note** in the same commit, naming which keyframe is which kind of evidence.
- [ ] **Step 7: Commit.**

**Acceptance:** the handoff test matches 8792/8800 (or records a D24-attributed difference); the standalone 9000 matches; the note records the inheritance; the exit-during-collapse and exit-during-pulse tests pass.

---

### Task 8: COMAN — the sinusfield

**Files:**
- Create: `bins/secondreality/src/parts/coman.rs`, `bins/secondreality/tests/coman.rs`, `docs/fidelity/3dsinfld.md`; modify `parts.rs`, `shell.rs` (music row), the `expected` table.

**Interfaces:**
- Consumes: `PartCtx`, `sr_assets` (`W1DTA.BIN`/`W2DTA.BIN` raw, `COMBG.UH` only if the note records the unlinked path).
- Produces: `pub struct Coman`, registered as `"3dsinfld"`.

**The transliteration (from the survey):**
- **Init** (`MAIN.C:118-227`): the palette formula (`uc=(223-a*22/26)*3`, green/blue/red diamond, brightness `*9/6` over bytes 0..719, the dark-red bar 232..255, entry 0 black, entries 240..255 from the 16-byte `combg`), the wave tables (the linked BINs; `CALCW12` dead), `inittwk`, the clear/upload.
- **`doit`** (`:45-116`): `waitb`, `musplus<0` wait, `frame<4444` loop, break `-8<musplus<0`, `frepeat` catch-up, `startrise`, the sine camera and 160-column `docol`, the two-page flip (`0x0c` 0x80/0x00), `docopy`'s 160→320 duplication and fixed clears.
- **`theloop`** (`THELOOP.INC`): the swapped-16.16 `eax`, `wflip` constants, `wave1[si]`/`wave2[di]` 16-bit wrap, `zwave[j]-240`, the 64 single + 64 double steps, the carry-dependent `_@seek1a`; the pinhole colour `((bx+140)&0xff)>>1`.
- **Bugs to preserve:** the unlinked `combg` and its 16-byte overrun; the palette aliasing; the dead frame-4 upload; the dead 4444 cap; `cameralevel` dead; `docol`'s `~1` masking; `docopy`'s fixed bands; the wave adjacency.
- **Reference:** 15500/16000/16500/16800 (RGB only), moving in two-frame steps; start window (15000,15100]; part-relative.

**Steps:**
- [ ] **Step 1: Write the failing tests** — the `wflip`/16.16 arithmetic; the ray-march's first column against the source's step sequence; `docopy`'s 160→320 duplication; the two-page flip.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (init, `doit`, `theloop`).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison** — 15500/16000/16500/16800.
- [ ] **Step 6: Write the note** in the same commit.
- [ ] **Step 7: Commit.**

**Acceptance:** the four keyframes; the swapped-16.16 and the dead paths pinned; the note records the background's absence from the link.

---

### Task 9: JPLOGO — the jelly logo

**Files:**
- Create: `bins/secondreality/src/parts/jplogo.rs`, `bins/secondreality/tests/jplogo.rs`, `docs/fidelity/jplogo.md`; modify `parts.rs`, `shell.rs` (music row), the `expected` table.

**Interfaces:**
- Consumes: `PartCtx`, `sr_assets` (the Origin RLE `ICEKNGDM.UP`, the `ZOOM.INC`-generated scale routines or an equivalent).
- Produces: `pub struct Jplogo`, registered as `"jplogo"`.

**The transliteration (from the survey):**
- **Init** (`JP.C:107-227`): the spring tables (200→800 interpolation, 1/16 px), `inittwk` (400-line planar), the palette from the UP file, the 64-black entry and source-0→64 remap, the 184-byte crop plus the 65 border, `scrolly(400→0)`'s 83 steps.
- **`doit`** (`:67-105`): the `musplus<4` wait, `frame<700`, the freeze after 511, `xsc`, the per-row `b=(y-y1)*400/(y2-y1)` and `a=184+(sin1024[b*32/25]*xsc+32)/64` rounded even, the dirty-row guard, `linezoom`.
- **The zoom tables:** `DOL.C`'s 140..244 even scales, `(x-le)*185 + c/2)/c`, the fixed 32-pixel margins, the per-plane writes.
- **Bugs to preserve:** `_setpalarea`'s 512-byte upload (invisible for the used indices but mandatory); JPLOGO's own sine table (index 256 = 255, 768 = −255); the crop/65 border; source 0→64; the OOB interpolation; the freeze's unused `c=400`; the palimpsest redraw guard.
- **Reference:** 17300/17500/17700 (RGB only); the three phases; the hold states; start (17000,17100]; part-relative; the 17700 state caveat (the filed picture is 0/4 by the census and is the recorded residual).

**Steps:**
- [ ] **Step 1: Write the failing tests** — the spring tables' first frames; the row-scale arithmetic; the 512-byte palette upload; the `scrolly` view.
- [ ] **Step 2: Run them; fail.**
- [ ] **Step 3: Implement the part** (init, `doit`, the zoom).
- [ ] **Step 4: Run the unit tests; pass.**
- [ ] **Step 5: The reference comparison** — 17300/17500/17700 with the 17700 state caveat.
- [ ] **Step 6: Write the note** in the same commit.
- [ ] **Step 7: Commit.**

**Acceptance:** the three keyframes with 17700's residual attributed; the spring/scale/palette bugs pinned; the note covers the hold states.

---

### Task 10: The shell — the six parts in their blocks

**Files:**
- Modify: `bins/secondreality/src/shell.rs` (`music_for_part` rows, the `expected` table, `boot_anchor` if any exact start is measured, the block-stop expectations), `bins/secondreality/tests/shell.rs` (block runs and the chain), `docs/fidelity/part-sequence.md` if a block's contents change.

**Interfaces:**
- Consumes: the six registry names and Task 1's measured orders; Task 3's seam.
- Produces: `--part` works for all six; `--block 3` runs MNTSCRL → LNS&ZOOM → PLZPART → MINVBALL → RAYSCRL → 3DSINFLD → JPLOGO and stops at U2E (unported); `--block 2` still stops at GLENZ; `--block 1` still stops at U2A.

**Steps:**
- [ ] **Step 1: `music_for_part` rows.** From Task 1's measured orders: `lnszoom` MUSIC1@45 (measured in `second3`), and the other five as measured/derived; record each as measured or derived. Extend the `expected` table; the registry-length assertion forces every name.
- [ ] **Step 2: Anchors.** None of the six has an exact first frame (all starts are windows); `boot_anchor` stays `None`, and the notes/status line say part-relative. If a start is measured exactly during the task, record it; do not infer.
- [ ] **Step 3: The block runs.** `--block 3` dumps a keyframe per part in demo order (MNTSCRL 1100, LNS 2700, PLZ 6000/8500, MINVBALL 10900, RAYSCRL 13000, COMAN 15500, JPLOGO 17300), asserting each part's name, numbering and music row; the stop test's block-3 row moves to `u2e`.
- [ ] **Step 4: The chain, with the seam.** A block-3 run reaches MINVBALL and RAYSCRL mid-chain; assert the real player's positions at those parts (`75:61`, `88:0` as the reference traces have them) and that their pre-rolls did not fire. This is the seam's end-to-end proof.
- [ ] **Step 5: Run the workspace suite and commit.**

**Acceptance:** the six `--part` runs; the block-3 chain test with the real player; the stop move; the music rows asserted.

---

### Task 11: The phase's verification and close-out

**Files:**
- Create/modify: `PROGRESS.md`, `docs/decisions.md`, `docs/fidelity/APPROXIMATIONS.md`, the six notes, `docs/fidelity/part-sequence.md`, `CLAUDE.md`.

**Steps:**
- [ ] **Step 1: Re-run every criterion at the committed state**, recording the numbers verbatim: each part's keyframes under its own invocation; the workspace tests; both clippy commands; `reference verify`; `assets verify --decode`.
- [ ] **Step 2: The falsification list** in Phase 2/3/4's shape: each constant or rule (the cue forms, the stage bounds, the two-screen transitions, the palette truncations, the seam's fresh/chain distinction, the table data), its derivation, what a one-unit error would change, and the strongest independent check — naming honestly which have none.
- [ ] **Step 3: The negatives**, at least: which parts' cue evidence rests on the three new traces and which on synthetic sequences; TECHNO/PANICEND's block-2 unreachability until GLENZ; PANICEND's predecessor dependence; the part-relative mappings; the RGB-only keyframes and the JPLOGO 17700 state caveat; the `3dsinfld` module-name split.
- [ ] **Step 4: The record**: `PROGRESS.md`'s Phase 5 row (complete, untagged), the evidence, the "what Phase 5 leaves Phase 6" list; `decisions.md` for the seam and the PLZPART data decision; `APPROXIMATIONS.md` for any D24 divergence; the six notes; `CLAUDE.md`'s part list and block examples.
- [ ] **Step 5: Nothing is tagged here.** The final whole-branch review runs first; the tag goes on after its findings are applied.

**Acceptance:** every number reproduced by the command the record names; the six notes complete; the suite green.

---

## Self-review notes (for the plan itself)

- **Coverage**: spec §10's Phase 5 exit criterion names exactly these six parts; each has a task; the traces, the carried fixes, the seam, the shell and the close-out are Tasks 1–3, 10–11.
- **Order**: Task 1 gates every part's music row and cue evidence; Task 3 gates the chain; the six parts are independent apart from the registry; Task 10 needs all six.
- **Known risks the plan carries open**: PLZPART's table-base displacement (`0xC100` etc.) must be verified against the map/binary; TECHNO's 5000/5050 palette hand-over needs a measurement to settle; PANICEND's 8792/8800 evidence is the handoff test, not a standalone comparison; the block-2 parts stay unreachable through a block until GLENZ (Phase 7); the seam changes `PartCtx::new`'s arity across 19 call sites.
- **Deliberate omissions**: GLENZ, U2A/U2E (Phases 7/6); the block-2→3 alignment wait (Phase 9); the full-run music restarts (Phase 9).
