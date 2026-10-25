# The music clock — what the reference's own player state says

Phase 3 has no audio reference: the emulator runs silent (D17 requires it, because a real
audio device makes the animation drift), and the two modules' *timing* is invisible from the
outside. What is visible is the player's own state. `MAIN/STMIK.H:125-131` publishes the words
STMIK keeps its position in, `MAIN/STMIK.300`'s OMF `PUBDEF` records give their offsets, and
`dosbox-automation` can read guest memory — so the oracle this phase builds is a **per-frame
trace of the original's own player state**, and Tasks 3, 6 and 8 are measured against it.

**Every number in this note is produced by a command named beside it** (D25), and the
traces below are committed under `reference/music/`:

| trace | command | what it is for |
|---|---|---|
| `reference/music/second5` | `cargo xtask music trace --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..1300 --out reference/music/second5` | the phase's oracle: the boot, the music's start, the first graphics frame, ENDLOGO's gate window |
| `reference/music/second3` | `cargo xtask music trace --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..2900 --out out/b3` | MUSIC1: its order list, and one of its `254`s |
| `reference/music/hiddenu` | `cargo xtask music trace --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..6500 --out out/u` | MUSIC0's orders 70…77, and the first session of recipe `U` (its 408 anchor) |
| `reference/music/second5b` | the same command as `second5`, into a fresh directory | **a second session of the same recipe**: the anchor's reproducibility (§2), and the machine's own words moving into their playing values (§5) |
| `reference/music/second1` | `cargo xtask music trace --arg 1 --cycles 30000 --frames 148..6000 --out reference/music/second1` | **`SECOND 1`'s boot**: ALKU's cue window, and the block-1 anchor — the initial start with no block restart (§2) |
| `reference/music/second1b` | `cargo xtask music trace --arg 1 --cycles 30000 --frames 7600..7960 --out reference/music/second1b` | PAM's `dis_sync()<10` wait and its 40 drawn frames (Phase 4) |
| `reference/music/second3b` | `cargo xtask music trace --arg 3 --cycles 30000 --frames 10200..15050 --out reference/music/second3b` | DOTS's and RAYSCRL's `dis_musplus` cue windows, past `second3`'s end (Phase 4) |
| `reference/music/hiddenu2` | `cargo xtask music trace --arg U --cycles 30000 --frames 6500..8200 --out reference/music/hiddenu2` | DDSTARS's 8000 keyframe and the stage marks around it (Phase 4) |
| `reference/music/second2` | `cargo xtask music trace --arg 2 --cycles 30000 --frames 4700..9300 --out reference/music/second2` | TECHNO's and PANICEND's start windows, in block 2's own session (Phase 5) |
| `reference/music/second3a` | `cargo xtask music trace --arg 3 --cycles 30000 --frames 2900..10200 --out reference/music/second3a` | LENS's and PLZPART's start windows; joins `second3` to `second3b` (Phase 5) |
| `reference/music/second3d` | `cargo xtask music trace --arg 3 --cycles 30000 --frames 15000..17800 --out reference/music/second3d` | COMAN's and JPLOGO's start windows (Phase 5); ends before the `:880-885` MUSIC0 restart |
| `reference/music/second1c` | `cargo xtask music trace --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 5900..7600 --out reference/music/second1c` | U2A's gate 1 (order 11 row 47) and gate 2 (order 12 row 55), in block 1's session (Phase 6) |
| `reference/music/second4` | `cargo xtask music trace --arg 4 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 500..4400 --out reference/music/second4` | the block-4 boot's MUSIC0@18 restart and U2E's order-19 gate (Phase 6) |
| `reference/music/second3e` | `cargo xtask music trace --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 18000..22000 --out reference/music/second3e` | the mid-run MUSIC0@18 restart and U2E's opening, on block 3's session (Phase 6) |
| `reference/music/second2b` | `cargo xtask music trace --arg 2 --cycles 30000 --frames 380..3800 --out reference/music/second2b` | **`SECOND 2`'s boot**: the block-2 BEGLOGO→GLENZ boundary, the GLENZ music-wait exit and the intro (Phase 7) |
| `reference/music/second1d` | `cargo xtask music trace --arg 1 --cycles 30000 --frames 8100..8800 --out reference/music/second1d` | the block-1 `MUSIC1@0` restart's post-reload session and GLENZ's block-1 wait exit (Phase 7) |
| `reference/music/second1e` | `cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..33950 --out reference/music/second1e` | block 1 from 8800 to JPLOGO: the rest of the `MUSIC1@0` session, PANICEND's cue wait; narrowed from the plan's `8800..<restart − 50>` (§2.3) (Phase 9) |
| `reference/music/second1f` | `cargo xtask music trace --arg 1 --cycles 30000 --frames 34105..48277 --out reference/music/second1f` | block 1 from the U2E `MUSIC0@18` restart + 50 to ENDSCRL's first frame + 2000 (§2.3) (Phase 9) |

The Phase 6 traces were captured with the same recipe as the first four, written out in their
rows above: `--boot autoexec --keys "KBD_left KBD_enter"` (D17) and `--cycles 30000`,
recorded in each manifest. The Phase 4 and Phase 5 traces use the same defaults and state
them once here. Their windows are the plan's, not widened to join
the existing traces: D19's deferred-read cost grows with the span.

**`second3d` is the plan's 15000..18000 narrowed to 15000..17800** (Phase 5's plan correction).
The planned tail crosses the `MAIN/U2.ASM:880-885` restart — `MUSIC0` at order 18, fired for
every block-3 run between JPLOGO and U2E — which reloads STMIK, and the trace command's
fixed-base read cannot follow it: the planned window ends with `_np_ord reads -1, which is not
a byte value: the base 0x0051F0 is not this segment's`. Live reads of the whole segment through
both views (`0x0051F0`, `0x1051F0`) were still valid at frame 17847, so the refusal is in
(17847, 18000); 17800 is below it. The omitted 17800..18000 is the loader/restart, after both
parts' start windows and every keyframe of both (COMAN 15500/16000/16500/16800, JPLOGO
17300/17500/17700), and belongs to the full-run phase (Phase 9's full-run restarts), not to
Phase 5.

The other command this note's numbers come from measures a word that no trace column carries
(§8):

```
cargo xtask music probe --arg 2 --cycles 30000 --frames 899..3000 --at 0x105B0E
```

The symbol map they read through, and the analysis every section below is printed by:

```
cargo xtask music symbols --object upstream/SecondReality/MAIN/STMIK.300
cargo xtask music analyse --trace reference/music/second5
cargo xtask music analyse --trace reference/music/second3
cargo xtask music analyse --trace reference/music/hiddenu
cargo xtask music analyse --trace reference/music/second1
cargo xtask music analyse --trace reference/music/second1b
cargo xtask music analyse --trace reference/music/second3b
cargo xtask music analyse --trace reference/music/hiddenu2
cargo xtask music analyse --trace reference/music/second2
cargo xtask music analyse --trace reference/music/second3a
cargo xtask music analyse --trace reference/music/second3d
cargo xtask music analyse --trace reference/music/second1c
cargo xtask music analyse --trace reference/music/second4
cargo xtask music analyse --trace reference/music/second3e
cargo xtask music analyse --trace reference/music/second2b
cargo xtask music analyse --trace reference/music/second1d
cargo xtask music analyse --trace reference/music/second1e
cargo xtask music analyse --trace reference/music/second1f
```

## 1. What the trace reads, and what it guarantees

| word | offset in `STMIK.300`'s segment | read as |
|---|---|---|
| `_np_zinfo` | 0x08F7 | 16-bit little-endian |
| `_np_zplus` | 0x08F9 | 16-bit |
| `_np_zframe` | 0x08FB | 16-bit |
| `_np_row` | 0x08FD | byte of the 16-bit word |
| `_np_pat` | 0x08FF | byte |
| `_np_ord` | 0x0901 | byte |
| `_np_loop` | 0x0903 | byte |
| `_np_tempo` | 0x090F | byte |
| `_np_speed` | 0x2002 | byte |

`cargo xtask music symbols` prints these and the other 36 published symbols with their
offsets, the record census (**278 records**: THEADR 1, COMENT 5, MODEND 1, **PUBDEF 45**,
LNAMES 2, **SEGDEF 1**, LEDATA 105, LIDATA 118), the one segment
(`text_stmik300_text`, class `CODE`, **29,607 bytes**, `acbp` 0x68) and the two needles. The
map is `D8`'s route 1 — the object's own symbols — and the seven position words are one
contiguous run of two-byte slots (`0x08F7` … `0x0903`), which the symbol test asserts slot by
slot because `MAIN/STMIK.H:125-131`'s declaration order is *not* the memory order.

Three things the trace guarantees, and why they matter:

- **It is keyed by the guest's own frame counter.** Every sample is bracketed by two reads of
  the counter and dropped when the counter moved between them; a sample that arrives late is
  one frame with a different number, never a wrong value under the right number.
- **One memory read per sample, of the whole segment.** Nothing the phase compares frame by
  frame can tear against anything else.
- **The segment is located by the object's own bytes, twice.** Two runs of 32 bytes with no
  byte repeated inside them, from different `LEDATA` records, must solve the *same* base; the
  scan is re-run at the end of every trace and the manifest records both.

## 2. Where the music starts

`cargo xtask music analyse --trace reference/music/second5`, section 1:

```
  first advancing frame: 410
    frame   407  ord   0 row   0 pat   0  zframe      0
    frame   408  ord   0 row   0 pat   0  zframe      0
    frame   410  ord   0 row   0 pat   0  zframe      1
    frame   411  ord   0 row   0 pat   0  zframe      2   <- zframe 1 -> 2
    frame   412  ord   0 row   0 pat   0  zframe      2
```

**The music boot anchor is frame 408 for the recipes that restart their player**, measured
two ways in four Phase 3 sessions. `music analyse` derives it from the trace's own columns and
prints both readings — the `second5` session:

```
  the anchor the rows imply: the first row change is at frame 427 with `_np_zframe` 13, and 13 ticks at the measured 1.4622 frames a tick is 19.0 frames — so that row began at frame 408.0 (± the tick the row boundary is within and the frame the change is seen on)
  the anchor the `zframe` column shows: frame 410 is the first frame whose `zframe` is not 0 — 2.0 frame(s), or 1.4 tick(s), from the rows' reading: the same tick, seen at the next whole-frame sample. Both readings are quantised to a whole frame, so the gap is one frame plus the tick the row change is seen within; it is not a claim that they agree to within a tick, and the two numbers above say how big it is.
```
(The tool prints each of those two readings as **one line**; they are pasted here as printed, not wrapped.)

The derivation uses no simulation and no free parameter: the first row change is at frame
427 with the *tick counter's own value* (13) there, and back-extrapolating 13 ticks at the
tick clock's measured 1.4622 frames a tick puts the first tick at **408.0**. Its uncertainty
is the tick the row boundary is within and the frame the change is seen on — the tool's own
words — and the sessions below show how little it moves within a recipe.

| trace | frames | module | session | the rows imply | `zframe`'s first non-zero frame |
|---|---|---|---|---|---|
| `second5` | 148..1299 | MUSIC0 | 1 | **408.0** | 410 |
| `second5b` | 146..1299 | MUSIC0 | 2 (the same recipe, a fresh run) | **408.0** | 410 |
| `second3` | 144..2899 | MUSIC1 | 3 | **408.1** | 411 |
| `hiddenu` | 148..6499 | MUSIC0 | 4 | **408.1** | 409 |
| `second1` | 148..5999 | MUSIC0 | 5 (`SECOND 1`) | **399.6** | 405 |
| `second1b` | 7600..7959 | MUSIC0 | 6 (a window) | *371.9* — an extrapolation, not a boot anchor | 7600 |
| `second3b` | 10200..15049 | MUSIC1 | 7 (a window) | *5727.6* — an extrapolation, not a boot anchor | 10200 |
| `hiddenu2` | 6500..8199 | MUSIC0 | 8 (a window) | *390.7* — an extrapolation, not a boot anchor | 6500 |
| `second2` | 4700..9299 | MUSIC1 | 9 (a window) | *1108.9* — an extrapolation, not a boot anchor | 4700 |
| `second3a` | 2900..10199 | MUSIC1 | 10 (a window) | *2450.8* — an extrapolation, not a boot anchor | 2900 |
| `second3d` | 15000..17799 | MUSIC1 | 11 (a window) | *5723.6* — an extrapolation, not a boot anchor | 15000 |
| `second1c` | 5900..7599 | MUSIC0 | 12 (a window) | *399.7* — an extrapolation; block 1's clock is 399.6 | 5900 |
| `second4` | 500..4399 | MUSIC0 | 13 (a window) | **408.2** — the block-4 boot's own MUSIC0@18 restart (§2.1) | 500 |
| `second3e` | 18000..21999 | MUSIC0 | 14 (a window) | *4718.9* — a mixed-tempo extrapolation that is **not** the restart's start (§2.1) | 18000 |
| `second2b` | 380..3799 | MUSIC1 | 15 (**`SECOND 2`'s boot**) | **401.1** | 408 |
| `second1d` | 8100..8799 | MUSIC1 | 16 (a window, after a reload) | *997.0* — an extrapolation across the `MUSIC1@0` reload, not a boot anchor | 8100 |
| `second1e` | 8800..33949 | MUSIC1 | 17 (a window, the same `MUSIC1@0` session) | *8523.4* — the last tick reset before the window, GLENZ's `dis_setmframe(0)` at 8521 (§2.2), not a boot anchor | 8800 |
| `second1f` | 34105..48276 | MUSIC0 | 18 (a window, after the U2E reload) | *20927.5* — an extrapolation over a `zframe` the restart does not zero (§2.1); the restart itself is 34,055 (§2.3) | 34105 |

Each row's frames are the manifest's own coverage (`first..last`), and its module the one
`music analyse` identifies from the trace's `(ord, pat)` pairs.

`second4`'s 408.2 is the one window row whose derivation is a real clock start rather than an
extrapolation: the block-4 boot's first show part is U2E, so the `MAIN/U2.ASM:880-885` MUSIC0@18
restart fires at the boot's first graphics frame, and the trace's whole window lies in that
restarted player's session. `second3e`'s 4718.9 is **not** an anchor of any kind: the
extrapolation multiplies all 9,106 ticks it reads at MUSIC0's tempo-120 rate, but the session
began under MUSIC1 at tempo 130, so the number is meaningless; §2.1 measures the restart's
start by the shift between the two sessions instead.

**The anchor is a property of the boot recipe, not of a session — and there is more than one
recipe.** The restart recipes agree on **408**: `second5` and `second5b` are the same recipe
twice and derive the same 408.0, and `second3` and `hiddenu` derive 408.1. `SECOND 1` is the
exception, and it is a measurement: **it derives 399.6, 8.4 frames before 408.** Block 1 never
takes a block restart — its row of `MAIN/U2.ASM:766-775` loads the module with `bx = -1`, "no
seek", and the play is the initial `_zplaysong(0)` at `:785-787` — while `SECOND 3`, `5` and
`U` take their own `restartmus`/`_zplaysong` paths at their starts (`:856-863`, `:898-905`,
`:889-894`). The eight-frame difference is a property of those recipes' start paths, not a
counter reset: **a restart does not zero `_np_zframe`** — `second3e` reads 9,099 at frame 18000, the
count carried since PLZPART's `_dis_setmframe(0)` at 5,723, through the U2E restart at ~17,846
(§2.1). A recipe's *first* play at boot is what starts from the static zero, which is why the
column reads 0 at the anchor in every boot trace. `music analyse`'s two readings for `second1`, section 1:

```
  the anchor the rows imply: the first row change is at frame 420 with `_np_zframe` 14, and 14 ticks at the measured 1.4583 frames a tick is 20.4 frames — so that row began at frame 399.6 (± the tick the row boundary is within and the frame the change is seen on)
  the anchor the `zframe` column shows: frame 405 is the first frame whose `zframe` is not 0 — 5.4 frame(s), or 3.7 tick(s), from the rows' reading: the same tick, seen at the next whole-frame sample. Both readings are quantised to a whole frame, so the gap is one frame plus the tick the row change is seen within; it is not a claim that they agree to within a tick, and the two numbers above say how big it is.
```

The direct reading is 5.4 frames from the rows' rather than the Phase 3 traces' 0.9…2.9
because the sample at 405 caught two ticks at once (`zframe` 0 → 2 between frames 404 and 405)
— the same host-timing term D17 measures, not a second clock.

**The window traces do not hold the start, and their printed "anchors" are
extrapolations, not boot anchors.** Each back-extrapolates the last tick-counter reset before
its window over thousands of ticks, using the frames a tick measured over the window's own
frames. The ten readings, each from `cargo xtask music analyse --trace reference/music/<name>`,
section 1, pasted as printed:

`second1b`:

```
  the anchor the rows imply: the first row change is at frame 7609 with `_np_zframe` 4939, and 4939 ticks at the measured 1.4653 frames a tick is 7237.1 frames — so that row began at frame 371.9 (± the tick the row boundary is within and the frame the change is seen on)
```

`second3b`:

```
  the anchor the rows imply: the first row change is at frame 10201 with `_np_zframe` 3323, and 3323 ticks at the measured 1.3462 frames a tick is 4473.4 frames — so that row began at frame 5727.6 (± the tick the row boundary is within and the frame the change is seen on)
```

`hiddenu2`:

```
  the anchor the rows imply: the first row change is at frame 6508 with `_np_zframe` 4191, and 4191 ticks at the measured 1.4596 frames a tick is 6117.3 frames — so that row began at frame 390.7 (± the tick the row boundary is within and the frame the change is seen on)
```

`second2`:

```
  the anchor the rows imply: the first row change is at frame 4701 with `_np_zframe` 2664, and 2664 ticks at the measured 1.3484 frames a tick is 3592.1 frames — so that row began at frame 1108.9 (± the tick the row boundary is within and the frame the change is seen on)
```

`second3a`:

```
  the anchor the rows imply: the first row change is at frame 2905 with `_np_zframe` 337, and 337 ticks at the measured 1.3478 frames a tick is 454.2 frames — so that row began at frame 2450.8 (± the tick the row boundary is within and the frame the change is seen on)
```

`second3d`:

```
  the anchor the rows imply: the first row change is at frame 15006 with `_np_zframe` 6888, and 6888 ticks at the measured 1.3476 frames a tick is 9282.4 frames — so that row began at frame 5723.6 (± the tick the row boundary is within and the frame the change is seen on)
```

`second1c`:

```
  the anchor the rows imply: the first row change is at frame 5901 with `_np_zframe` 3769, and 3769 ticks at the measured 1.4596 frames a tick is 5501.3 frames — so that row began at frame 399.7 (± the tick the row boundary is within and the frame the change is seen on)
```

`second4`:

```
  the anchor the rows imply: the first row change is at frame 506 with `_np_zframe` 67, and 67 ticks at the measured 1.4603 frames a tick is 97.8 frames — so that row began at frame 408.2 (± the tick the row boundary is within and the frame the change is seen on)
```

`second3e`:

```
  the anchor the rows imply: the first row change is at frame 18009 with `_np_zframe` 9106, and 9106 ticks at the measured 1.4595 frames a tick is 13290.1 frames — so that row began at frame 4718.9 (± the tick the row boundary is within and the frame the change is seen on)
```

`second1d`:

```
  the anchor the rows imply: the first row change is at frame 8102 with `_np_zframe` 5278, and 5278 ticks at the measured 1.3462 frames a tick is 7105.0 frames — so that row began at frame 997.0 (± the tick the row boundary is within and the frame the change is seen on)
```

`second3b`'s 5727.6 and `second3d`'s 5723.6 are the same reset — PLZPART's
`_dis_setmframe(0)` at its own start (`PLZPART/PLZ.C:69`) — and `second3a` shows that reset
inside its own window at frame 5723 (`_np_zframe` reads 0). `second3a`'s 2450.8 is LNS's
`_dis_setmframe(0)` (`LENS/MAIN.C:106`), which `second3` shows at frame 2452. `second2`'s
1108.9 is the last reset before its window, which the window does not contain — that reset is
GLENZ's `dis_setmframe(0)`, and `second2b` (§2.2) measures it at **1111**. **`SECOND 2`'s boot
is now measured**: `second2b` opens at 380, before the first tick, and its own columns derive
**401.1** by the rows with the `zframe` column first non-zero at 408 — a **6.9-frame gap**, the
largest in the table. The gap is the boot's first ticks seen at nearly one a frame: `zframe`
goes 0 → 2 between 407 and 408 and 2 → 14 from 408 to 420, while the row reading applies the
steady 1.3474 frames a tick. The recipe's seek is the shell's own start,
`restartmus(1, 0, 48700)` (`MAIN/U2.ASM:766-775`) followed by `_zplaysong(0)` (`:785-787`),
MUSIC1 at order 0; `SECOND 2` therefore derives **401.1**, not the restart recipes' 408, and
the trace test simulates it from that measured anchor (`crates/sr-audio/tests/trace.rs`). Its
direct `zframe` reading is the restart recipes' 408 while its row reading is block 1's
neighbourhood (399.6), so the two readings straddle both recipes' clocks: that is what makes
401.1 a measurement of `SECOND 2`'s own path rather than an assumption.

`second1d`'s 997.0 is the same kind of artefact as `second3e`'s 4718.9 and is recorded because
the tool prints it: its window begins at 8100 after the block-1 `MUSIC1@0` reload, `_np_zframe`
carried across the load (a restart does not zero it), and the extrapolation therefore back-dates
the count to a frame before the demo's music at all. It is not a boot anchor; §2.2 records what
the window does measure.

Phase 6's three windows read differently by kind. **`second1c`'s 399.7 lands on block 1's
399.6** — a window whose session never reset, projected onto a rate measured over its own
frames, 0.1 frame from the recipe's boot anchor. **`second4`'s 408.2 is a real start, not an
extrapolation**: the block-4 boot's first show part is U2E, so the `MAIN/U2.ASM:880-885`
restart fires at the boot's first graphics frame and the whole window is that one restarted
player's session at one tempo; its reset is the `_zplaysong(18)` itself. **`second3e`'s 4718.9
is meaningless** and is recorded only because the tool prints it: the extrapolation applies
MUSIC0's tempo-120 rate to ticks earned partly under MUSIC1 at tempo 130, and the counter was
not zeroed at the restart anyway (§2.1 measures the restart's start by the shift between
`second4` and `second3e` instead). The trace test treats all three as windows and simulates
each from a start that is a real clock: recipes 1 and 4 from their boot/restart anchors, and
`second3e` from the restart's measured 17,850 through the trace's own restart table.

`second1b` and `hiddenu2` hold no reset inside their window and their projections land near
their recipes' anchors (390.7 against 408, 371.9 against 399.6) rather than at a part start.
**They therefore neither confirm nor contradict 408**: `hiddenu2`'s 390.7 is 17.3 frames below
its recipe's 408 and `second1b`'s 371.9 is 27.7 below its recipe's 399.6, because the rate each
window measures over its own frames (359 for `second1b`, 1,699 for `hiddenu2`) multiplied by
4,939 and 4,191 ticks respectively carries the measurement's error with it. A window's columns
cannot measure a boot anchor; the trace test (`crates/sr-audio/tests/trace.rs`) simulates the
port from the recipe's anchor and phase-aligns it at the window instead.

**What the windows do measure is the session phase spread.** `hiddenu` ends at frame 6499
reading `_np_zframe` 4175 at ord 28 row 9; `hiddenu2` begins at 6500 reading 4185 at ord 28
row 11. The two sessions of one recipe are **ten ticks — about fifteen frames — apart** at the
join, which is D17's host-timing term accumulated over the boot.

The Phase 5 windows measure the same term at their joins. `second3` ends at 2899 on ord 48 row
37 with `_np_zframe` 331 and `second3a` opens at 2900 on ord 48 row 38 with 333 — the row
boundary and a caught second tick fall in the one-frame gap. `second3a` ends at 10199 on ord 75
row 19 with 3321 and `second3b` opens at 10200 on the same ord 75 row 19 with 3322 — one tick
apart, the same position. `second3d` overlaps `second3b` at frames 15000..15049, and there the
two sessions differ: `second3b` reads ord 91 at 15000 and crosses to 93 at frame 15031, while
`second3d` reads ord 91 (row 59, `_np_zframe` 6884) at 15000 and crosses at 15038 — **seven
frames apart at the same order boundary**.

The trace test shifts each window's port run by the offset its first row change measures
(−16 frames for `hiddenu2`, −5 for `second2`, +1 for `second1b` and `second3b`, +2 for
`second3a`, +4 for `second3d`) before comparing it.

`xtask/tests/music.rs`'s `the_anchor_is_the_same_in_every_filed_session` asserts the
per-recipe anchors, walked from `reference/music/` so a trace filed later is covered without
an edit, and refuses to treat a window's extrapolation as one; §7 is why the traces disagree
about the frame the tick is first *visible* on.

Everything is still before it: `ord`, `row` and `pat` are 0, `zframe` is 0, and `speed` 6 /
`tempo` 125 are the object's own initial values (`cargo xtask music symbols` prints the
object; `np_tempo`'s static byte is 0x7D = 125, ST3's default —
`crates/sr-assets/tests/omf_symbols.rs`'s `the_objects_own_initialisers_are_the_notes_values`). The first *row* advances at
frame 427, when the shell has restarted the song at order 25 (`MAIN/U2.ASM:902-905` is the
order-25 restart, `mov bx,25` at `:903`; `restartmus(0, 25, 45000)`).

### The order in force at each Phase 5 part's start

Task 10's `music_for_part` rows consume the order each part hears when its start window opens.
The start windows are the plan's (`docs/plans/05-mid-tier.md`'s part table); the orders are the
traces' own `ord` column at those frames, read with these commands (the columns are the
manifest's):

```
awk -F, '$1 == 2452' reference/music/second3/trace.csv
awk -F, '$1 >= 5720 && $1 <= 5730' reference/music/second3a/trace.csv
awk -F, '$1 == 4797' reference/music/second2/trace.csv
awk -F, '$1 >= 8780 && $1 <= 8790' reference/music/second2/trace.csv
awk -F, '$1 >= 15000 && $1 <= 15100' reference/music/second3d/trace.csv
awk -F, '$1 >= 17000 && $1 <= 17100' reference/music/second3d/trace.csv
```

| part | start window | trace | the order in force |
|---|---|---|---|
| LNS&ZOOM (`lnszoom`) | (2452, 2456] | `second3` | MUSIC1 **45** — frame 2452, row 59, pattern 33; `_np_zframe` reads 0 there (the part's `_dis_setmframe(0)`, `LENS/MAIN.C:106`) |
| PLZPART | (5720, 5730] | `second3a` | MUSIC1 **62** — from frame 5723 (row 0, pattern 43, `_np_zframe` 0, the part's `_dis_setmframe(0)`, `PLZPART/PLZ.C:69`); the window's first sampled frame 5720 is still order 60 row 63, the natural advance over the `254` at 61 landing with the reset |
| TECHNO | 4797 | `second2` | MUSIC1 **18** — frame 4797, row 60, pattern 3; `_np_zframe` reads 0 at frame 4794 (the part's `_dis_setmframe(0)`, `TECHNO/KOE.C:214`) |
| PANICEND | (8780, 8790] | `second2` | MUSIC1 **37** — frames 8780..8790, rows 53..55, pattern 16; the part has no music call of its own |
| COMAN (`3dsinfld`) | (15000, 15100] | `second3d` | MUSIC1 **91 at the window's opening, 93 from frame 15038** — 91 (rows 59..63, pattern 37) at 15000..15037, and the crossing over the `254` at 92 into 93 (row 0, pattern 68) at 15038; COMAN has no `setmframe` and nothing in the trace marks its first frame, so the single `music_for_part` value is **derived, not measured**: 91, the order in force at the window's opening |
| JPLOGO | (17000, 17100] | `second3d` | MUSIC1 **96** — the whole window (frames 17000..17100, rows 50..62, pattern 71); the counter crosses over the `254` at 97 to 98 at frame 17109, nine frames after the window |

The orders for LNS&ZOOM, PLZPART, TECHNO, PANICEND and JPLOGO are measured at their start
windows; COMAN's single value is the one derivation in the table, and it is named as one.

### The orders in force at U2A's and U2E's starts, and the U2E restart's clock

Phase 6's two vector parts gate on absolute positions, and the three Phase 6 traces measure
them. Every frame below is a `frame` column the file itself carries.

**U2A (block 1, `SECOND 1`).** The gate is `order>10 && row>46` (`VISU/C/U2A.C:229-239`), so
the first frame that satisfies it is the first that publishes order 11 row 47:

```
$ awk -F, '$2 == 11 && $3 == 47' reference/music/second1c/trace.csv | head -1
6988,11,47,8,0,6,120,0,0,4513
```

Gate 1 passes at **frame 6988** (order 11 row 47, pattern 8, `_np_zframe` 4513) in
`second1c` (frames 5900..7599, 1,699 samples, one gap at 5958). Gate 2's condition
`order>11 && row>54` first holds at order 12 row 55, 19 frames after `second1c`'s window
closes; `second1b` (7600..7959) holds it:

```
$ awk -F, '$1 == 7618' reference/music/second1b/trace.csv
7618,12,55,11,0,6,120,0,0,4945
```

so gate 2's first satisfying frame is **7618** (order 12 row 55, pattern 11, `_np_zframe`
4945). That is a **music-only value**: U2A's animation stream ends first (~7510), so the
reference reaches gate 2 in the trace but the part normally retires at the stream's END
before it. `second1c`'s orders advance at 6016 (10), 6576 (11) and 7136 (12), and it holds
MUSIC0 orders 9…12.

**U2E (block 4's boot; block 3 after the restart).** The gate is `order>18`
(`VISU/C/U2E.C:358-366`), so the first satisfying frame is the first that publishes order 19:

```
$ awk -F, '$2 == 19' reference/music/second4/trace.csv | head -1
699,19,0,51,0,6,120,0,0,199
```

**frame 699** (order 19 row 0, pattern 51), in `second4` — the block-4 boot's own session.
The plan's four sampling frames, as the file carries them:

```
$ awk -F, '$1 == 1000 || $1 == 1600 || $1 == 2600 || $1 == 3600' reference/music/second4/trace.csv
1000,19,34,51,0,6,120,0,0,405
1600,20,38,49,0,6,120,0,0,816
2600,22,25,52,0,6,120,0,0,1501
3600,24,11,47,0,6,120,0,0,2187
```

The row cadence, from `cargo xtask music analyse --trace reference/music/second4`, section 3:

```
  frames 506..3772 at speed 6 tempo 120: 373 gap(s) — 8 frame(s) x91, 9 frame(s) x282
  frames 3781..3934 at speed 7 tempo 120: 15 gap(s) — 10 frame(s) x12, 11 frame(s) x3
  frames 3945..4120 at speed 8 tempo 120: 15 gap(s) — 11 frame(s) x5, 12 frame(s) x10
  frames 4131..4395 at speed 6 tempo 120: 30 gap(s) — 8 frame(s) x7, 9 frame(s) x22, 10 frame(s) x1
```

— one row is 8.76 frames at speed 6, 10.22 at 7 and 11.68 at 8. At speed 6 the orders start
every 560–561 frames (699, 1259, 1820, 2380, 2940, 3501), and order 25 starts at 4131 after
the order-24 speed excursion. `second4` holds MUSIC0 orders 18…25.

**The mid-run restart's clock (`second3e`).** `second3e` opens at 18000, inside the MUSIC0@18
session the shell installed at the U2E boundary (`MAIN/U2.ASM:880-885`), and holds the same
orders 18…25. Its first sample and the plan's two comparison frames:

```
$ awk -F, '$1 == 18000 || $1 == 18100' reference/music/second3e/trace.csv
18000,18,16,50,0,6,120,0,0,9099
18100,18,27,50,0,6,120,0,0,9168
```

**18:16 at 18000 and 18:27 at 18100**; `second4` publishes the same two positions 17,442
frames earlier, at 558 and 658. Every order boundary of the two sessions differs by the same
17,441–17,442 frames:

| boundary | `second4` | `second3e` | difference |
|---|---|---|---|
| order 19 | 699 | 18140 | 17,441 |
| order 20 | 1259 | 18701 | 17,442 |
| order 21 | 1820 | 19261 | 17,441 |
| order 22 | 2380 | 19822 | 17,442 |
| order 23 | 2940 | 20382 | 17,442 |
| order 24 | 3501 | 20943 | 17,442 |
| order 25 | 4131 | 21573 | 17,442 |

```
$ awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second4/trace.csv
$ awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second3e/trace.csv
```

The two are one restart recipe: `second4`'s restart is the block-4 boot's, fired at the boot's
first graphics frame, and `second3e`'s is the shell's at the U2E boundary, **17,442 frames
later at the rows measured here** (the seven order boundaries sample 17,441–17,442, a whole
frame each). `second4`'s own columns derive that session's clock start at **408.2** (the
restart recipes' 408), so `second3e`'s restarted clock starts at **408.2 + 17,442 = 17,850.2**
(±1 frame), and the
boundary's first graphics frame is four frames earlier at **17,846.2** — §2's 404→408
relation. That is D34's waiting=4 verified at the chain's mapping: chain tick 0 is boot 419
(MNTSCRL's tick 0, D31 §3), so the boundary is chain **17,427** and the restarted music's
first tick is chain 17,431 = boot 17,850.

**What `_np_zframe` does across the restart, measured.** `second3e` reads **9099** at frame
18000, which is not a fresh count: `restartmus` does not zero the word. The measured trail
from the previous window: `second3d` reads **8,961 at 17,799**; then 47 frames at MUSIC1's
tempo-130 rate (~35 ticks), the **4 wait frames** of the `restartmus`/`_zplaysong` boundary
(0 ticks — the player is not polled across them), and 150 frames at MUSIC0's tempo-120 rate
(~103 ticks) give **8,961 + 35 + 0 + 103 = 9,099**. No later part reset it. The restart's
phase
is therefore carried by the order/row words the parts read, not by `zframe`; a port player
that starts its own counter at 0 at the restart differs in that word only, which neither
vector part's gate reads (both read order/row through `int 0fch` fn 6).

**The screens agree as the music does.** `reference-indices.md` §18.1's byte identity holds
across the two boots: `out/review-b4b/s3-after`'s 18000 and 18100 are `befaf38a…`, the same
bytes as the `SECOND 4` boot's opening hold (`out/b4b-u2e-sweep2`'s 530 = 590 = 660). The
positions above are that one screen at the same phase in the two sessions.

### The block-2 and block-1 GLENZ boundaries (Phase 7)

`reference/music/second2b` is `SECOND 2`'s boot from frame 380, and it holds both sides of the
block-2 BEGLOGO→GLENZ boundary. The port's block-2 boundary is frame **576** — BEGLOGO's 172
ticks after its own 404 (`docs/fidelity/beglogo.md` §3: `FADE_FIRST_TICK` 42 + 129 fade steps +
the final write) — and the trace reads MUSIC1 there:

```text
$ awk -F, '$1 == 576' reference/music/second2b/trace.csv
576,0,40,73,0,3,130,0,0,129
```

GLENZ's wait (`GLENZ/MAIN.C:290`) is `while(!dis_exit() && dis_musplus()<-19)`, and `musplus` is
`musplus_from(zplus, row)` (`crates/sr-engine/src/music.rs`). At 576 `zplus` is 0, so the −32
sentinel holds the wait; the first published value that opens it is order 2 row 45 with `zplus`
1 — row − 64 = **−19** — and `dis_setmframe(0)` runs on that frame:

```text
$ awk -F, '$1 >= 1109 && $1 <= 1112' reference/music/second2b/trace.csv
1109,2,44,50,0,3,130,0,1,525
1110,2,44,50,0,3,130,0,1,526
1111,2,45,50,0,3,130,0,1,0
1112,2,45,50,0,3,130,0,1,1
```

**The reset is frame 1111** (`_np_zframe` 526 → 0), the zoomer's first tick. A capture of the
same boot — `out/p7-collapse2`, `--frames "$(seq -s, 1100 1154)"` — is byte-identical through 1110 and
first changes on **1111** (320 pixels), so trace and picture agree on the frame in two sessions.
The intro's two `getmframe` waits then read:

```text
$ awk -F, '$1 >= 1513 && $1 <= 1517' reference/music/second2b/trace.csv
1513,5,16,28,0,3,130,0,0,298
1514,5,16,28,0,3,130,0,0,299
1515,5,16,28,0,3,130,0,0,300
1516,5,17,28,0,3,130,0,0,300
1517,5,17,28,0,3,130,0,0,301
$ awk -F, '$1 >= 1558 && $1 <= 1562' reference/music/second2b/trace.csv
1558,5,27,28,0,3,130,0,0,332
1559,5,27,28,0,3,130,0,0,332
1560,5,28,28,0,3,130,0,0,333
1561,5,28,28,0,3,130,0,0,334
1562,5,28,28,0,3,130,0,0,335
```

`getmframe()<300` first sees 300 on 1515 (then `dis_waitb` on 1516 and 1517 — the tmppal build
and the `bgpic` snapshot); `getmframe()<333` first sees 333 on 1560 (then the third
`dis_waitb` on 1561), and the main loop's first `dis_waitb` is 1562. The measured loop start
below is **L_r = 1562**: main-loop frame f is the picture at reference `1562 + f`.

`reference/music/second1d` is the block-1 boundary. The wide window `7800..8800` **refused**
across the reload — `error: _np_ord reads -1, which is not a byte value: the base 0x0051F0 is
not this segment's` (exit 2) — because the block-1 `restartmus` (`MAIN/U2.ASM:813-818`) reloads
the player's segment. The last pre-reload window, `7800..8050` into `out/second1d-limit`, passes,
and reads MUSIC0 at 8049 (`8049,14,4,31,0,6,120,0,0,5239`); the filed window opens at 8100 with
MUSIC1:

```text
$ head -2 reference/music/second1d/trace.csv
frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe
8100,1,4,39,0,3,130,0,0,5276
```

The restart is therefore in **(8049, 8100)**, and the filed window holds the *post-reload*
player, not the seek's first row. GLENZ's block-1 wait opens on the same published condition as
block 2's — order 2 row 45, `zplus` 1, `musplus` −19 — and `dis_setmframe(0)` runs there:

```text
$ awk -F, '$1 == 8520 || $1 == 8521' reference/music/second1d/trace.csv
8520,2,44,50,0,3,130,0,1,5588
8521,2,45,50,0,3,130,0,1,0
```

**The block-1 reset is frame 8521**, and the picture agrees to the frame: `out/p7-b1-collapse`
(the same `--arg 1` boot, frames 8440…8550 ten apart) is BEGLOGO's held 900 screen byte for
byte through 8520 and changes by 8530, and the dense `out/p7-b1-collapse-dense` (frames
8516…8528) first changes on **8522** — one frame after the trace's reset sample, the
sample/publish instant D17 quantises, where on `SECOND 2` the two both read 1111. From the
filed window's first sample to the reset the music makes **105 row changes** (`ord 1 row 4` →
`ord 2 row 45`) in 421 frames — 315 ticks at 3 a row against the 312 sampled `_np_zframe`
ticks plus the reset tick.

**One reading is a limit, not an explanation.** The post-reload sample at 8100 is 68 rows into
MUSIC1 (order 0's 64 rows and order 1's first 4), and at the module's own 1.3474 frames a tick
that play clock began ~275 frames before 8100. The part chain puts GLENZ's start later than
that: BEGLOGO's final screen is up through 8520, its white field is flat 251 at 7880–7900 and
its 320×400 mode change is at 7920 (`out/p7-b1-boundary`, scratch, regenerated by
`cargo xtask reference capture --arg 1 --cycles 30000 --boot autoexec --keys "KBD_left KBD_enter"
--frames 7780,7800,7820,7840,7860,7880,7890,7900 --out out/p7-b1-boundary`; `docs/fidelity/pam.md` §6.5), and its
172 ticks put the block-1 boundary around 8060. The trace's order/row words and the part
chain's frames therefore do not agree on when the restarted player began to play, and the trace
cannot separate a seek that left the row from a play that began during the module load. The
wait's **exit** (8521) is what Task 6 consumes and it is measured twice; the restart frame is
recorded as the interval **(8049, 8100)**. `docs/decisions.md` D38 carries the consequence.
*(Phase 9 Task 6 found, and Task 6b fixed, why the two readings disagreed: a mid-song restart keeps
the previous module's channel volumes (D45), so MUSIC1's order 0 is four rows long here, not 64, and
the sample at 8100 is 8 rows into the restarted module, not 68. With the port's restarted player the
two agree: the restart is at **8,056**, inside the interval; see "The block-1 full run against the
port".)*

### The block-1 full run: `second1e` and `second1f` (Phase 9)

Phase 9 Task 2 traces block 1 from 8800 to ENDSCRL, split at the U2E restart because a trace cannot
follow a reload (§1's fixed-base read, the `second3d` and `second1d` refusals above).
`reference-indices.md` §26.6 carries the record; the numbers are these.

**`second1e`'s window is narrowed.** The plan's end is the restart − 50, 34,005. Two windows were
refused before one passed:

```
$ cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..34070 --out reference/music/second1e
error: _np_ord reads -1, which is not a byte value: the base 0x0051F0 is not this segment's
$ cargo xtask music trace --arg 1 --cycles 30000 --frames 8800..34005 --out out/p9-trace-second1e-b
error: the needle is in 2 places and the bytes around them are NOT one memory: the copy at 0x1051F0 differs from the one at 0x0051F0 at offset 0x0068 (192 against 64), so one of them is a stale copy and a trace read through it would be a record of dead bytes
```

and `8800..33950` passes — so the reload is under way by the end-of-trace re-scan that follows
34004, and the filed window ends 105 frames before the restart. It holds MUSIC1 orders 4…98 and, at
33693–33872, speed 127.

**The U2E restart's clock starts at 34,055 ± 2**, read as `second3e`'s was (§2.1): `second1f`'s rows
at order 18 sit 33,644–33,647 frames after `second4`'s, so 408.2 + 33,647 ≈ 34,055. The order
boundaries then drift: orders 19…25 start 33,646, 33,644, 33,642, 33,641, 33,639, 33,636 and 33,635
frames after `second4`'s, and the offset stays at 33,635 to the window's end —

```bash
awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second1f/trace.csv
awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second4/trace.csv
```

— 11 frames over the 3,421 frames of orders 19–25, where `second3e` against `second4` held
17,441–17,442 at every boundary. `second1f`'s first attempt, `--frames 34170..48280` into
`out/p9-trace-second1f-a` (exit 0), agrees with the filed one at every row change within −2…+3.

**PANICEND's cue wait (`MAIN/U2.ASM:833-853`) exits at 16630.** `musplus` from `second1e`'s columns
is −32 from 16363 to 16618, 0 at 16622 (order 40 breaks to order 42 row 0) and **1 at 16630**
(order 42 row 1), the first value above 0 after the negative stretch — the exit, whichever of the
wait's two loops PANICEND's retirement entered.

**What the port's player does with them** (`cargo test -p sr-audio --test trace`, both in
`RESTART_TRACES`): **`second1e`'s content is identical** — every word's value sequence over 25,136
frames — **and its placement drifts to 8 frames**, one past `JITTER_FRAMES`: the worst displacement
first reaches 1 at 8,813, 4 at 16,190 and 8 at 33,305 (the comparison prints each; Task 2 measured
1, 4 and 8 at 10,827, 19,981 and 33,264 from the 64-row restart anchor Task 6b replaced);
**`second1f` has one row the trace never publishes**:
order 26 runs at **speed 1** (frames 38328–38370, 1.46 frames a row, the first speed-1 stretch in
any filed trace), the trace publishes row 25 at 38364 and row 27 at 38366, and the port, ten frames
later in phase, publishes row 26 at 38376 (`second1f: the row sequence differs at index Some(502):
trace 27, port 26`), and its placement drifts with the port late by up to 12 frames. The player was
not changed then; Task 6b's restart carry-over (D45), under which both traces are simulated through
the session their restart replaces, leaves every `second1f` figure as it was and `second1e`'s worst
at 8.

**Both are pinned, exactly, in `KNOWN_DISAGREEMENTS`** (`crates/sr-audio/tests/trace.rs`), and
**Task 6 registered both rows as `APPROXIMATIONS.md` A54** — neither is a seam (below). `the_port_reproduces_every_filed_trace` holds a
listed trace to its recorded disagreement rather than to the general rule, and fails if the
measurement gets better or worse; every other trace keeps the general rule:

| trace | content differences | frames where the port publishes a value the trace never does | worst displacement |
|---|---|---|---|
| `second1e` | none | none | **8** (`JITTER_FRAMES` is 7) |
| `second1f` | one: `row`, index 502, trace 27, port 26, every other value of every word identical | **38376** (order 26 row 26) | **12** |

`cargo test -p sr-audio --test trace` checks the table. Changing a recorded value makes it fail:
with `second1e` set to 7 and `second1f`'s frame list emptied, it fails with `second1e: the worst
displacement is 8 frames, not the recorded 7` and `second1f: the port publishes a value the trace
never does at [38376], not the recorded []`. Every row must name a trace the walk found
(`every_known_disagreement_names_a_trace_the_walk_found`).

### The block-1 full run against the port (Phase 9 Task 6; re-measured by Task 6b)

The port's own `--block 1`, played headless to 48,277 with `--positions`, against block 1's six
traces (`cargo test --release -p secondreality --test fullrun -- --nocapture`; the table
is `reference-indices.md` §26.8's). One offset per trace — 0 for the boot trace, and for a window the
port's change to the trace's first row change minus the trace's — then every frame within the
shell's `JITTER_FRAMES` = 3:

```text
second1: offset +0 at 420 (0, 1); 5399 of 5541 frames within 3 (97.437 %), worst 7, 0 past 60; with no offset 5399 (97.437 %); offset +3 at its last row change (5994)
second1c: offset -1 at 5901 (9, 51); 1699 of 1699 frames within 3 (100.000 %), worst 1, 0 past 60; with no offset 1698 (99.941 %); offset -2 at its last row change (7592)
second1b: offset -1 at 7609 (12, 54); 358 of 359 frames within 3 (99.721 %), worst 1, 1 past 60; with no offset 357 (99.443 %); offset -2 at its last row change (7951)
second1d: offset +5 at 8102 (1, 5); 699 of 699 frames within 3 (100.000 %), worst 2, 0 past 60; with no offset 227 (32.475 %); offset +5 at its last row change (8797)
second1e: offset +5 at 8801 (4, 50); 18928 of 25143 frames within 3 (75.281 %), worst 8, 0 past 60; with no offset 23083 (91.807 %); offset -3 at its last row change (33872)
second1f: offset -8 at 34108 (18, 5); 6352 of 14168 frames within 3 (44.833 %), worst 12, 1 past 60; with no offset 13554 (95.666 %); offset +0 at its last row change (47969)
```

The boot's MUSIC0 is the original's to the frame through BEGLOGO; the `MUSIC1@0` session from GLENZ
to JPLOGO is **5 frames late** at its first rows (−3 at `second1e`'s last) and the `MUSIC0@18`
session after U2E **8 early** (0 at the last row change the run reaches). Task 6 measured +247, +248
and +235 here, before the restart kept the old module's channel state. The cue wait exits on the
original's row (`42:1`, the port's 16632 against `second1e`'s 16630).

**Why the `MUSIC1@0` session was late, and the fix (A38, D45).** It was neither a seek that left the
row nor a play during the load: `restartmus`'s `_zplaysong` keeps the channel volumes the previous
module left (the song initialiser at `+0x5614` and its silence passes write `achannelused`,
`lastins`, the periods, the cell and the remembered parameters, and never `avol`, `a0volcut` or the
effect counters), MUSIC1's order 0 (pattern 73) has a `C00` on channel 2 at row 3, and a fresh
player's "volume 0" countdown frees that channel on row 3 before its command runs — which is why
block 2's boot (`second2b`) plays order 0's 64 rows. Block 1's MUSIC0 has channel 2 at 63 when it
hands over, so the original takes the break and plays order 0 for 4 rows; the port's restart now
continues from the old player (`Player::continue_from`) and does the same, publishing `1:0` at
8,087. Back from `second1d`'s first `1:5` (8,102) the restarted player's 46th poll puts the trace's
restart at **8,056** (`RESTART_TRACES`), where the 64-row reading put it at 7,814 — inside BEGLOGO,
while `second1b` still publishes MUSIC0 at 7,959:

```bash
cargo test -p sr-audio --test player the_song_initialiser_writes_exactly_what_initialise_writes
cargo test -p sr-audio --test trace block_ones_music1_restart_takes_order_0s_break -- --nocapture
```

**The residual +5** is the restarted clock's: the port installs the restart at its boundary,
8,057, and D34's four-tick wait starts its clock at 8,061, five frames after the trace's 8,056 —
D34's relation is the restart recipes' boot one (404 → 408), and a mid-run restart's reload is not
modelled (A53). It is A38's measured residual, not fitted away.

## 3. The tick rate, and the rule behind it

A tick is `2.5/tempo` seconds and a row lasts `speed` ticks, so the row rate is
`speed × tempo/2.5` rows a second — **a prediction from the player's own `_np_speed` and
`_np_tempo` words, not a fit to the trace**, and the words are what the trace reads rather
than what a file says (for MUSIC1 the two disagree, and the trace's word is the one the rate
follows — the first bullet below). The module headers are the context: MUSIC0's carries speed
6 and tempo 125, and `MAIN/STARTMUS.C:257` patches its tempo byte to 0x78 = 120 before playing
it; MUSIC1's carries speed 3 and tempo 130.

`cargo xtask music analyse` measures each run of constant speed and tempo separately (the
modules change both as they play, `Axx`/`Txx` cells), between the run's first and last row
change so that the part-row at each end does not enter the residual:

| trace | run | row intervals | frames | predicted | residual | one row |
|---|---|---|---|---|---|---|
| `hiddenu` | frames 430…3885, speed 8 tempo 120 | 296 | 3455 | 3457.6 | **−2.6** | 11.68 |
| `hiddenu` | frames 3897…6119, speed 6 tempo 120 | 254 | 2222 | 2225.2 | **−3.2** | 8.76 |
| `second1` | frames 420…5994, speed 6 tempo 120 | 637 | 5574 | 5580.6 | **−6.6** | 8.76 |
| `second5` | frames 427…961, speed 6 tempo 120 | 61 | 534 | 534.4 | **−0.4** | 8.76 |
| `second3` | frames 427…2472, speed 6 tempo 130 | 253 | 2045 | 2046.0 | **−1.0** | 8.09 |
| `second3` | frames 2488…2896, speed 3 tempo 130 | 101 | 408 | 408.4 | **−0.4** | 4.04 |

The longest window available — `second1`'s 637 row intervals over 5574 frames — is reproduced
to **−6.6 frames, 0.12 %**; the previous longest, `hiddenu`'s 296 over 3455 frames, to −2.6,
0.08 %, and **every steady run's residual is under one row**. The residual grows with the
window because the reference's measured tick rate is not exactly `2.5/tempo`
(48.06 a second against 48.00 at tempo 120 in `second1`), which is A12's subject. The two runs
that are not steady are the two that follow a module-driven speed change, and they are the
third bullet below: `second5` frames 1024…1295 (−35.6, one row 8.76) and `hiddenu` frames
6182…6495 (−37.4). The rate is the module's own rule and nothing else.

**Two things the traces say that the files do not.**

- **The player's speed word, not the module's header byte, is what the row rate follows, and
  the word's initial value after a restart is not the file's byte.** `second3` is MUSIC1 — its
  `(ord, pat)` pairs match `assets/MAIN/MUSIC1.S3M`'s order list at every order the counter
  held (`analyse` identifies the module that way, not by speed) — and for the orders 42…45 it
  holds after the `SECOND 3` restart (frames 427…2472) the trace's own `_np_speed` reads
  **6**, where that file's header byte 0x31 is **3**: the measured row interval is 8.09
  frames, speed 6 at tempo 130, where speed 3 would be 4.04. From frame 2488 — the crossing
  into order 47 — the word is **3** and the interval follows it (4.04 frames a row, residual
  −0.4, the table's last line). So the row rate follows the trace's own word in both halves
  of the same session, and what writes the initial 6 — rather than the header's 3 — is not
  measured here.
- **A speed change is followed by a transient.** `second5` at order 25 row 63 sets speed 1
  (the module's own cell): 29 row intervals in 42 frames over frames 979…1021 (residual −0.3
  — one row per tick, exactly as the rule says). When the speed returns to 6 at frame 1023
  the trace's third run measures 35 row intervals in 271 frames where the rule predicts 306.6
  — **−35.6 frames**, with five 1–2 frame gaps among the usual 8s and 9s. `hiddenu` shows the
  same transient at its own speed-1 row (−37.4 over 40 intervals). The port must reproduce
  it; Task 3's frame-by-frame comparison is where it will show.

## 4. The quantisation

The row-change frames are whole frames: the tick clock (48.0 Hz at tempo 120, 52.0 Hz at 130)
and the video clock (70.086 Hz) are not commensurate, so a row boundary lands on whichever
frame is current when the player is polled. `cargo xtask music analyse`, section 3, prints the
gap histogram per run:

| trace | run | gaps |
|---|---|---|
| `hiddenu` | frames 430..3885, speed 8 tempo 120 | 11 frames ×97, 12 ×199 |
| `hiddenu` | frames 3897..6119, speed 6 tempo 120 | 8 ×64, 9 ×190 |
| `second5` | frames 427..961, speed 6 tempo 120 | 8 ×16, 9 ×44, 10 ×1 |
| `second5` | frames 979..1021, speed 1 tempo 120 | 1 ×16, 2 ×13 |
| `second3` | frames 427..2472, speed 6 tempo 130 | 8 ×232, 9 ×21 |
| `second3` | frames 2488..2896, speed 3 tempo 130 | 3 ×1, 4 ×95, 5 ×5 |

Two gap sizes carry every steady run — the intervals take two neighbouring whole-frame
values and alternate between them — and the sizes outside the two the run's own row length
brackets are **few and scattered through the run, not gathered at its edges**. `music
analyse` prints them with their positions; the two runs below are `second5`'s first
(`cargo xtask music analyse --trace reference/music/second5`) and `second3`'s second
(`cargo xtask music analyse --trace reference/music/second3`), and `(…)` marks the lines
between them that are not shown:

```
  frames 427..961 at speed 6 tempo 120: 61 gap(s) — 8 frame(s) x16, 9 frame(s) x44, 10 frame(s) x1
    distinct gap sizes: 3 (one row is 8.8 frame(s) at this speed/tempo)
    the 1 gap(s) outside the 8..9 the run's own row length (8.76 frames) brackets, by their position in the run: 18 of 61 (575 -> 585, 10)
(…)
  frames 2488..2896 at speed 3 tempo 130: 101 gap(s) — 3 frame(s) x1, 4 frame(s) x95, 5 frame(s) x5
    distinct gap sizes: 3 (one row is 4.0 frame(s) at this speed/tempo)
    the 1 gap(s) outside the 4..5 the run's own row length (4.04 frames) brackets, by their position in the run: 48 of 101 (2678 -> 2681, 3)
```

`second5`'s 10-frame gap is the 18th of that run's 61 intervals and `second3`'s 3-frame gap
the 48th of 101 — both with ordinary intervals on either side. **Why they happen is not
measured here**, and Task 3 must not read §4 as "only the ends are special": the trace's own
row-change frames are the acceptance test, not any rule stated over them.

Two runs in these traces have their outliers bunched at the start, and the tool prints both —
`second5`'s post-speed-change transient (§3) and the same run in `second5b`:

```
    the 6 gap(s) outside the 8..9 the run's own row length (8.76 frames) brackets, by their position in the run: 1 of 35 (1024 -> 1025, 1), 2 of 35 (1025 -> 1027, 2), 3 of 35 (1027 -> 1028, 1), 4 of 35 (1028 -> 1030, 2), 5 of 35 (1030 -> 1031, 1), 27 of 35 (1215 -> 1225, 10)
```

and `hiddenu`'s, where all five are at positions 1…5 of 40 (`1 of 40 (6182 -> 6183, 1)`, …
`5 of 40 (6188 -> 6189, 1)`). So "at the start" describes the *transient* runs, not the steady
ones: which of them a run is, and where its outliers fall, is what the command prints. That is what Task 3's player has to reproduce
tick for tick.

`second3d` measures a speed change to the module's own `A7F` cell (MUSIC1 order 98 row 48, the
cell `crates/sr-audio/tests/player.rs`'s row-timer check names). The published speed word
changes 6 → 127 at frame 17488; `cargo xtask music analyse --trace reference/music/second3d`,
section 2, measures the run 17489..17668 as 2 row intervals in 179 frames where
`speed × tempo/2.5` predicts 342.3 — an 8-frame interval and a 171-frame one, the 127-tick row.
The port reproduces it: the trace test's `second3d` line agrees on 2,716 of 2,792 frames with a
worst displacement of 2 (`cargo test -p sr-audio --test trace -- --nocapture`).

## 5. Which soundcard and mixer rate the reference runs at

`cargo xtask music analyse`, section 4, records each word **twice** — the reading taken when
the segment was located and the reading taken at the end of the trace. The pairs below are the
ones `second5b`'s manifest carries (`second5b` is the first filed trace with `machine_moved`,
and its segment was found at frame 144, §9; the four Phase 4 and three Phase 5 traces carry the
same four moved words); `second5`'s manifest, found at frame 147, records only the locate
reading:

| word | at the locate | when the music plays | what it is |
|---|---|---|---|
| `_zoutputmode` | **5** | 5 | Sound Blaster Pro (`MAIN/STMIK.H:67-74`) |
| `_zmixspeed` | 20000 | **40000** | the mixing rate: 20 kHz before the demo's music engine starts, **40 kHz** from the music's start on (`cargo xtask music probe --arg 5 --cycles 30000 --frames 408..1300 --at 0x5AC5` reads 40000 at both ends of that window) |
| `_zbufsize` | **4096** | 4096 | the DMA buffer, the largest `MAIN/STMIK.H:88-95` lists |
| `_zpollmix` | 0 | **1** | **the caller polls the player** (`MAIN/STMIK.H:152-176`): `1` means the program must call `zpollme`, which is what `MAIN/COPPER.ASM:466` does once per copper frame — `second5b`'s manifest records the move (0 at the locate, 1 at the end), and `music analyse` prints that pair on the word's own moved line |
| `_zspeedadjust` | **0** | 0 | |
| `_zspeedadjust2` | **0** | 0 | `MAIN/U2.ASM:389` writes it only for outputmode 7 or 2, and the card is 5 |
| `_np_mastervol` | 192 | **64** | the object's own initialiser is 0xC0 = 192 (`STMIK.300`'s static byte at segment offset 0x0911; `crates/sr-assets/tests/omf_symbols.rs` asserts it), and `fademusic` (`MAIN/U2.ASM:333-346`) walks it down as a part ends — 64 at the end of the window |
| `_np_mastermul` | 0 | **2240** | 0 before the module is loaded, then `0x8C0` = bit 11 (stereo) with a 192 multiplier once it plays |

**This settles spec §7.2's default from the machine rather than from `MENU.C`'s menu order:**
one of the four rates the shell offers is 20000 Hz and another is 40000 Hz
(`MAIN/U2.ASM:737-743`), the demo holds 20000 when its music engine is initialised, and the
**quarter of a megabyte per second is what the music itself runs at**. The tick clock does not
depend on it — a tick is `2.5/tempo` seconds of samples at *whatever* rate the mixer runs, so
the rate cancels (§3's residuals are 0.06-0.08%) — but the port's `MixRate` default is this one.

**And `_zpollmix = 1` is the port's own model confirmed from the machine**: the reference is
the *polled* mode, not the timer-interrupt mode, so the tick clock advances once per copper
frame's `zpollme` call — which is why `_np_zframe`'s rate (§8) quantises onto the frame clock
the way §4 measures, and why the port's `poll()` per logic tick is the right shape.

Nothing here is assumed constant. A word whose two readings differ is recorded **twice**, in
`machine` and `machine_moved` — `second5b`'s manifest carries both, and so do the four Phase 4
traces and the three Phase 5 ones — because which words move is not a rule this tool gets to
assume: the mixing rate moves, `zpollmix` moves, and the volume words move.

## 6. `zplus` and `zinfo`

`zplus` takes **0, 1 and 2** and is a function of the position: over `hiddenu`'s 626 distinct
`(ord, row)` pairs, **0** carry more than one value (`second5`: 131 pairs, 0; `second3`: 358
pairs, 0). Its transitions are at order starts, and they straddle the two modules' `254`
entries:

| trace | transitions `(ord, row, from, to)` |
|---|---|
| `second5` | (26, 0, 0→1), (28, 0, 1→2) — MUSIC0's `254` is at order 27 |
| `hiddenu` | the same two |
| `second3` | (45, 0, 0→1), (47, 0, 1→2), (48, 0, 2→1) — MUSIC1's `254` is at order 46 |

### The rule the port computes, and the command that checks it

The trace is the port's evidence, not its rule: the port has to **compute** `zplus`, and the
rule it computes is the original player's own arithmetic. This note first stated that rule as a
*function of the order sequence the trace shows* — "2 on a crossing … **unless** the entry after
the one entered is itself a `254`, in which case 1" — and that "unless" clause has **no cause in
the code**: the player ORs two bits and clears neither. Task 3 read the routine out of the object
and the clause is withdrawn.

The order advance is `STMIK.300` + `0x442d`. It clears `zplus` (`+0x442d`), then walks the order
list one entry at a time, and per entry examined:

```
  4452: 3c fe                 cmp    $0xfe,%al
  4454: 75 06                 jne    0x445c
  4456: 2e 83 0e 0d 09 01     orw    $0x1,%cs:0x90d      ; the *next* entry is a 254
  445c: 80 fb fe              cmp    $0xfe,%bl
  445f: 75 08                 jne    0x4469
  4461: 2e 83 0e 0d 09 02     orw    $0x2,%cs:0x90d      ; this entry is a 254 …
  4467: eb cc                 jmp    0x4435              ; … and is stepped over
```

So, **as the port's rule**:

- **bit 0** is set while the order being played has a `254` as its *next* entry in the module's
  order list — the marker is coming;
- **bit 1** is set when the order played was reached by stepping over a `254`;
- so a plain advance reads **0** or **1**, a marker crossing reads **2**, and a crossing into an
  order whose own successor is a `254` reads **3** — both bits, because nothing clears one;
- and a seek or a restart **credits no marker behind it**: the scan starts at the order it was
  given and looks only forward, which is why `SECOND 3`'s restart at order 42 — whose list
  predecessor is a `254` the player never played — reads 0.

`cargo xtask music analyse` re-derives it from each trace's own order sequence and prints the
count of samples it had to be right about — **0 mismatch(es) in every filed trace**, 52,517
samples across the sixteen (`1150`, `1152`, `2755`, `6351`, `5797`, `359`, `4846`, `1699`,
`4597`, `7298`, `2799`, plus Phase 6's `1699`, `3899`, `3999` and Phase 7's `3418`, `699`), each
trace's own line carrying its own count. A trace filed from a later window begins mid-song, so
the order it begins on has no entry transition inside the window: `hiddenu2` starts on MUSIC0
order 28 reading **2**, the crossing of the `254` at order 27, and `music analyse` seeds that
first order's expected value from the trace's first sample rather than calling the inherited
state a mismatch. Every later order is computed from an entry the trace holds, and a trace that
holds the start seeds the pre-music zero. The `second3` session's own line:

```
  the port's zplus rule, re-derived from this trace's own order sequence: 2755 sample(s) checked, 0 mismatch(es)
    rule: bit 0 set while playing an order whose next entry in the module's order list is 254; bit 1 set when the order played was reached by stepping over a 254 (so a plain advance reads 0 or 1, a marker crossing 2, and a crossing into an order whose own successor is a 254 reads 3, because the object ORs the two bits and clears neither); a seek or a restart credits no marker behind it
    the orders the counter held, in order: [0, 42, 43, 44, 45, 47, 48]
```

**The `2 → 1` the note used to explain with the "unless" clause is a plain advance.** `second3`
goes from order 47 to order 48, and 48's successor at 49 is a `254`, so bit 0 is set on the way
in: **1**, with no exception needed. The shape the clause *would* have mattered for is
`MUSIC1`'s orders 25 (`254`), 26 (pattern 13) and 27 (`254`): the crossing into 26 sets bit 1
and 26's own successor sets bit 0, so the object reads **3** there. **No filed trace holds that
order**, so the old reading and the object's both satisfy "0 mismatches" — which is exactly why
the object, and not the count, is what decides it (`xtask/src/music.rs`'s
`the_zplus_rule_reproduces_the_traces_own_order_sequences` now pins `3` for that shape, on
`MUSIC1`'s own order list).

**What is extrapolation, said plainly.** No trace holds an order two steps past a marker
**whose own successor is not a marker**, where the bit must fall from the crossing's `2` back to
`0`: `MUSIC0`'s crossing is measured (`second5` and `hiddenu` both hold order 28) but neither
holds order 29, and `MUSIC1`'s is measured (`second3` holds order 47) with order **48** two past
the marker at 46 — but 48's successor is a `254`, so it tests bit 0, not the clearing. So **`2`
returning to `0` on the next natural advance is not measured**; it is what the code does — the
advance clears `zplus` at `+0x442d` and rebuilds it — and the first trace that plays past a
marker without another one behind it will test it. Nor does any trace play a `255` (§10), so
nothing here says what `zplus` does there.

`muscode_6` (`DIS/DISINT.ASM:242-282`) shows how the *parts* use the value — a row offset
computed from `zplus` 0/1/2 for `dis_muscode` — and that is a reading of the source, not a
measurement of this rule; the rule above is the object's, checked against every sample of every
filed trace.

`zinfo` is **0 in every filed trace** — the sixteen manifests record the same word the trace
does, and `cargo xtask music analyse`, section 5, prints each trace's own range: one line per
run, `zinfo runs: 1 … 0`. The windows are the 1,300-frame recipes (`second5`, `second5b`), the
2,900-frame one (`second3`), the 6,500-frame one (`hiddenu`), Phase 4's four
(`second1` 148…5999, `second1b` 7600…7959, `second3b` 10200…15049, `hiddenu2` 6500…8199),
Phase 5's three (`second2` 4700…9299, `second3a` 2900…10199, `second3d` 15000…17799),
Phase 6's three (`second1c` 5900…7599, `second4` 500…4399, `second3e` 18000…21999) and
Phase 7's two (`second2b` 380…3799, `second1d` 8100…8799),
covering MUSIC0's orders 0…31 (9…12 in `second1c`, 18…25 in `second4` and `second3e`) and
70…77, and MUSIC1's 0, 18…98 (the `254` entries stepped over, §10).
**These captures therefore do not measure what its other values are**, and `D8`'s ladder is
answered only in its first step: route 1 (read the object's symbols, then the words live) was
performed and produced a map and a trace, and the word it watches stayed 0. `D8` now records
the outcome and the substitute the port uses; this note does not guess at a rule the traces do
not show.

## 7. What a trace does not hold, and why it moves between sessions

`music analyse` prints each trace's coverage, recomputed from the trace file's own frames, and
the note quotes it rather than counting in prose — one `second5` line the long way, then every
trace's line as the tool prints it:

```
  frames 148..1299 of the requested 147..1299: 1150 sample(s), 3 frame(s) missing (147, 409, 1220) (frames 147..147 of them are the boundary: the segment was located after they had gone) — this trace is NOT per-frame (recomputed from the trace file's own frames)
```

| trace | the tool's coverage line |
|---|---|
| `second5` | `frames 148..1299 of the requested 147..1299: 1150 sample(s), 3 frame(s) missing (147, 409, 1220) (frames 147..147 of them are the boundary: the segment was located after they had gone) — this trace is NOT per-frame (…)` |
| `second5b` | `frames 146..1299 of the requested 144..1299: 1152 sample(s), 4 frame(s) missing (144..145, 409, 1220) (frames 144..145 of them are the boundary: …)` |
| `second3` | `frames 144..2899 of the requested 142..2899: 2755 sample(s), 3 frame(s) missing (142..143, 417) (frames 142..143 of them are the boundary: …)` |
| `hiddenu` | `frames 148..6499 of the requested 146..6499: 6351 sample(s), 3 frame(s) missing (146..147, 410) (frames 146..147 of them are the boundary: …)` |
| `second1` | `frames 148..5999 of the requested 148..5999: 5797 sample(s), 55 frame(s) missing (407, 464, 466..467, 476, 502, 511, 513, 521..527, 529, 531, 541, 544..546, 548, 572, 3837, 3860, 3873, 3876, 3879, 3914..3917, 3921, 3926, 3953, 4017, 5567..5569, 5573..5575, 5584, 5590, 5595, 5620, 5626, 5648, 5668, 5674, 5684, 5707, 5710, 5712, 5955) — this trace is NOT per-frame (…)` |
| `second1b` | `frames 7600..7959 of the requested 7600..7959: 359 sample(s), 1 frame(s) missing (7920) — this trace is NOT per-frame (…)` |
| `second3b` | `frames 10200..15049 of the requested 10200..15049: 4846 sample(s), 4 frame(s) missing (10365, 11563, 13574..13575) — this trace is NOT per-frame (…)` |
| `hiddenu2` | `frames 6500..8199 of the requested 6500..8199: 1699 sample(s), 1 frame(s) missing (7335) — this trace is NOT per-frame (…)` |
| `second2` | `frames 4700..9299 of the requested 4700..9299: 4597 sample(s), 3 frame(s) missing (4796, 8192, 8781) — this trace is NOT per-frame (…)` |
| `second3a` | `frames 2900..10199 of the requested 2900..10199: 7298 sample(s), 2 frame(s) missing (5722, 8315) — this trace is NOT per-frame (…)` |
| `second3d` | `frames 15000..17799 of the requested 15000..17799: 2799 sample(s), 1 frame(s) missing (17056) — this trace is NOT per-frame (…)` |
| `second1c` | `frames 5900..7599 of the requested 5900..7599: 1699 sample(s), 1 frame(s) missing (5958) — this trace is NOT per-frame (…)` |
| `second4` | `frames 500..4399 of the requested 500..4399: 3899 sample(s), 1 frame(s) missing (4320) — this trace is NOT per-frame (…)` |
| `second3e` | `frames 18000..21999 of the requested 18000..21999: 3999 sample(s), 1 frame(s) missing (21761) — this trace is NOT per-frame (…)` |
| `second2b` | `frames 380..3799 of the requested 380..3799: 3418 sample(s), 2 frame(s) missing (405, 1158) — this trace is NOT per-frame (…)` |
| `second1d` | `frames 8100..8799 of the requested 8100..8799: 699 sample(s), 1 frame(s) missing (8568) — this trace is NOT per-frame (…)` |
| `second1e` | `frames 8800..33949 of the requested 8800..33949: 25143 sample(s), 7 frame(s) missing (12207, 15603, 16192, 21926, 24520, 26570, 33261) — this trace is NOT per-frame (…)` |
| `second1f` | `frames 34105..48276 of the requested 34105..48276: 14168 sample(s), 4 frame(s) missing (34116, 37967, 38569, 46264) — this trace is NOT per-frame (…)` |

**A count of the absent frames belongs to that line and not to this sentence.** Among the
Phase 3 four, `second5b` misses four where the other three miss three, and its two kinds are
the tool's own: the first run is the boundary the locate leaves — the frames "the segment was
located after they had gone", one frame in `second5` (`147..147`) and two in each of the other
three (`144..145`, `142..143`, `146..147`) — and the middle one is a frame the guest's counter
moved across, with the manifest carrying what the counter did around it — for `second5`,
`skipped` records `409..409 (the counter went 408 -> 410)` and `1220..1220 (the read that was
opened on frame 1220 was still open when the guest moved to 1221)`. **Which index falls at that
point is session-dependent** — 409 (`second5`, `second5b`), 417 (`second3`), 410 (`hiddenu`) —
and all of them sit at the same point in the demo, the boot's first graphics frame (the
reference's own captures put that at 404, D17).

**The Phase 4, 5, 6 and 7 windows have no boundary run.** Their segments were located at
frames 88…104 (Phase 4), frame 90 (`second2`, `second3a`), frame 92 (`second3d`), frames 89
(`second1c`, `second4`), frame 87 (`second3e`, `second2b`) and frame 88 (`second1d`), before
each window began, so the coverage lines start at the requested first frame. Their
absent frames are the read-latency kind D19 describes, scattered through the run: `second1`
misses 55 (45 carry a `skipped` entry — a read that arrived late or a counter that stepped —
and the other ten are frames the tool's re-taken read moved past, which the coverage recomputes
and the manifest lists without a `skipped` entry), `second1b` 1 (frame 7920, the counter went
7919 → 7921), `second3b` 4 (10365, 11563, 13574..13575), `hiddenu2` 1 (7335), `second2` 3
(4796, 8192, 8781), `second3a` 2 (5722, 8315), `second3d` 1 (17056, the counter went
17055 → 17057), `second1c` 1 (5958, the counter went 5957 → 5959), `second4` 1 (4320, the
counter went 4319 → 4321), `second3e` 1 (21761, the counter went 21760 → 21762), `second2b` 2
(405 — the boot's first graphics frame region, the counter went 404 → 406 — and 1158, the
GLENZ mode change, the counter went 1157 → 1159) and `second1d` 1 (8568, the counter went
8567 → 8569). **`second1e` and `second1f` (Phase 9) put eleven more on the same footing**: their
missing frames are where the frame captures of the same run cannot grab one either —
`reference-indices.md` §26.2's refused indices 16192 and 24520 exactly, and 15603, 21926, 26570,
33261, 34116 and 37967 at the same part loads — and 12207 is TECHNO's start (block 2's `second2`
misses 4796, 7,410 earlier). **`second2b`'s 1158 is the hypothesis's clearest case after `second5`'s 1220**:
it sits exactly at GLENZ's `mov ax,13h` (`GLENZ/MAIN.C:295-299`), where the reference's own
captures cannot grab 1155…1159 either, and both are a video-mode stop that publishes no frame.
Apart from that, none of them is the boot's first graphics frame: `second1`'s are spread from
407 to 5955, which is what a window nearly twice as long as Phase 3's costs in deferred reads.

**Why is a hypothesis.** What is measured is the counter's own readings, and they say only
that it went from one value to another with the frame between never current. The explanation
— that a video-mode change publishes no frame for a moment, so the emulator's counter steps
over one (D17 measured the same behaviour: "a video-mode stop or reset calls `GFX_EndUpdate`
without publishing") — is consistent with the Phase 3 traces and with where they all sit, and
is **not** measured here. `second5`'s 1220 has the same shape at the point the reference's own
index table puts the ENDLOGO → CRED change in (1180, 1440]
(`reference/parts/endlogo/manifest.json`, `reference/parts/cred/manifest.json`, both
`--arg 5`), and is not measured either. The Phase 4, 5 and 6 windows' absent frames need no
such hypothesis: their manifests say which read was dropped, and they sit where the host missed
it.

The tool does not call such a trace per-frame, and a trace that quietly sampled every other
frame is refused the same description — the check is recomputed from the trace's own frames
by `reference verify`, field by field, rather than believed.

## 8. `_np_zframe`, and the word `reference-indices.md` §1.7 measured

`_np_zframe` advances one per **tick**, not per frame: it is the word the parts set and wait
on (`DIS/DISC.ASM:138-159`, `DIS/DISINT.ASM:412-425`), it starts from 0 when the music does
(0 at frame 408, 1 at 410 in `second5`), and **it is reset mid-song**: `second3` goes
1516 → 0 at frame 2452 with the same pattern still playing (`ord 45`, `pat 33`, the row going
59 → 60 across the reset), so no song restart is visible there — what fits is
`_dis_setmframe(0)` (`DIS/DISC.ASM:138-148`), which the parts call at their own start
(`LENS/MAIN.C:106`, `GLENZ/MAIN.C:291`, `TECHNO/KOE.C:214`, `PLZPART/PLZ.C:69`).
`cargo xtask music analyse` therefore measures it over the longest run in which it never went
backwards, and counts the resets:

| trace | longest run | ticks | frames | per frame | per second | the tempo's rule |
|---|---|---|---|---|---|---|
| `second5` | frame 410 (1) → 1299 (609) | 608 | 889 | 0.6839 | **47.93** | 48.00 (tempo 120) |
| `hiddenu` | frame 409 (1) → 6499 (4175) | 4174 | 6090 | 0.6854 | **48.04** | 48.00 (tempo 120) |
| `second3` | frame 411 (2) → 2451 (1516) | 1514 | 2040 | 0.7422 | **52.01** | 52.00 (tempo 130) |

Two tempos and the tick rate each time — `_np_zframe` is the tick clock, and its rate is a
second, independent way of measuring what §3 measures through the row rate.

`docs/fidelity/reference-indices.md` §1.7 records a different word: a 16-bit word at physical
address **`0x105B0E`** advancing **0.7424 times a video frame**, reading 371/373/374 at video
frame 900. `cargo xtask music probe --arg 2 --cycles 30000 --frames 899..3000 --at 0x105B0E`
measures it — **and `_np_zframe` beside it, in the same bracketed samples**, so the two are
comparable frame for frame. The block below is that command's output **as the tool prints it**
(one line per reading; nothing re-wrapped), from four idle runs at the fix wave's tip:

```
address 0x105B0E: the segment's base(s) are 0x0051F0, 0x1051F0 (of 29607 byte(s))
  in the view at 0x1051F0 it is that base + 0x091E
  the last published symbol at or below it is _zmixercount (base + 0x091C, 2 byte(s) below); the next above is _zchn at base + 0x0DA0
  frame 900: 370; frame 2999: 1927 — +0.7418 per frame, +51.99 per second at 70.086 Hz
  2099 sample(s) between them
  _np_zframe over the same window: 1 at frame 1112 to 1401 at frame 2999 — +0.7419 per frame, +52.00 per second (1 reset(s))
  the player's own words at frame 2999: tempo 130, speed 3, zframe 1401, ord 11, row 0
```

**What reproduces, and what does not.** Three of the four idle runs printed every line above
verbatim; the fourth differed on one figure only — `_np_zframe`'s first frame was **1111**
(rate 0.7415, 51.97) rather than 1112, which is D17's host-timing term reaching even an idle
session. The last line read **`ord 11, row 0` in all four runs**. An earlier draft of this note
printed `ord 10, row 63` there: that was a reading from a different session, and it is recorded
here rather than deleted — the command does not reproduce it at this tip.

**The equality is idle-host, not absolute.** Under concurrent host load the same command moves
the frame column: with eight busy loops running, one run gave `frame 904: 374; frame 2999: 1934 —
+0.7446 per frame` and `ord 11, row 2`, and the final review's loaded runs read
`1927/1931/1939/1928` at frame 2999 with rates `0.7418/0.7437/0.7456/0.7423`. What the block
supports is the **rate**, not the digits: the two words advance at the same rate to within
0.2 % in every run recorded here (this fix wave's five and the review's eight), which is the
same clock; where a given frame sits is the emulator's host-timing term (D17), not a property
of the port or of the word.

- **It is not `_np_zframe`** (`0x08FB`), and it is not a different *clock* either: over the
  same 2,099 frames the two advance at the same rate to four places — **51.99 a second and
  52.00 a second**, which is what `2.5/tempo` gives at the tempo the same output reports,
  **130**. §1.7's name for the word was right about what it is (a counter on the player's
  tick clock) and wrong about which word it is: offset `0x091E` is an **unnamed word** two
  bytes past `_zmixercount` (base + `0x091C`, whose two-byte word ends at `0x091D`), and a
  thousand bytes below the next published symbol, `_zchn`.
- **What tells the two apart is a reset, not a rate.** `_np_zframe` is zeroed by whoever calls
  `_dis_setmframe(0)` (`DIS/DISC.ASM:138-148`), which parts do at their own start; the 0x091E
  word is not. In that window `_np_zframe` is reset once (its longest run starts at frame
  1112) and ends at 1401, while the 0x091E word runs 370 → 1927 without a break — and the
  difference, 1927 − 1401 = **526**, is the ticks the reset cost: the 709 frames from the
  music's start (≈403, §2) to the reset (≈1112) at 0.7418 a frame is 526. So `_np_zframe` is
  the parts' copy of the player's tick count and 0x091E is the count itself; which one a port
  needs is Task 3's business, and §8's table above is `_np_zframe`, the word the parts read
  through `dis_getmframe`.
- **§1.7's numbers reproduce at that address**: the rate to three parts in ten thousand
  (0.7418 against 0.742016/0.742012/0.741909) and the value at frame 900 (**370** against
  371/373/374, which is §1.7's own one-to-two-frame spread between sessions, D17).

## 9. The segment's two views

The locator's needles are found at **two** addresses, exactly 1 MB apart — `0x0051F0` and
`0x1051F0` in the block-5 trace — and they are one memory: the whole 29,607 bytes read
through each base are byte-identical at the frame the segment is located and again at the end
of the trace, and the two views advance in lockstep over thousands of frames. The mechanism
is measured in the emulator's own source: its memory read goes through the page tables
(`dosbox-automation`, `hardware/memory.cpp:199` → `cpu/paging.h:293`), so two linear addresses
can be one physical page, and the demo's DOS extender maps it twice.

`music trace` accepts several views **only** when that equality is proved, and refuses a
second copy that differs (a stale copy would read as a perfectly good segment base until the
player moved). The trace reads through the **lowest** view, and the manifest records every
view, the proof, and the frame it was measured at. `music analyse --at` tries every view, so
§1.7's address is placed in the view it was measured in.

**Three of the Phase 3 manifests are older than the tool that reads them, in three ways, and
they are left as they were: `second5`, `second3` and `hiddenu`** — the three taken before those
fields existed (`second5b`, filed last in Phase 3, carries `coverage.before_sampling` and a
`locate.aliasing` sentence with one `0x`). They carry no `coverage.before_sampling` (§7: the
field that names the boundary run); their `locate.aliasing` sentence still contains a doubled
`0x` that the tool no longer writes; and they carry no `machine_moved` either, so their
`machine` map holds the reading at the locate and whatever the machine did later is not in
their record. `second5b` — the same recipe as `second5` — is the first manifest that carries
the moved words (four of them), and the four Phase 4 and three Phase 5 manifests carry the same
four (§5);
`music probe --arg 5 --cycles 30000 --frames 408..1300 --at 0x5AC5`
reads the mixing rate at 40000 in that recipe. None is repaired here: `reference/` is input-only,
a *new* directory under `reference/music/` is the only thing `music trace` may write inside it,
and hand-editing a filed manifest is what Phase 2's `reference backfill` exists to avoid being
done by a person. What keeps the filed records checkable is the reading side: `reference verify`
recomputes each trace's coverage from the trace file itself and compares it **field by field**
— and only compares `before_sampling` when a manifest records one — while `music analyse`
recomputes the coverage too, so the boundary is named for these traces as well (§7's block is
that output). A field a manifest predates is D23's additive case, not a defect.

## 10. The order list: what the player does at `254` and `255`

`cargo xtask music analyse`, section 6, reads this off the order counter's own sequence:

| entry | what the trace shows |
|---|---|
| MUSIC0 `254` at order 27 | **stepped over**: the counter went ord 26 → ord 28 without resting on 27, having played each of them (`second5`, frame 1260; the same in `hiddenu`) |
| MUSIC1 `254` at order 46 | **stepped over**: ord 45 → ord 47 without resting on 46 (`second3`, frame 2488) |
| MUSIC0's six `255`s (orders 16, 17, 64, 69, 78, 79) | **not played**: the shell's restarts jump over 16 and 17 (ord 0 → ord 25 at frame 427) and order 70 (ord 28 → 70 in `hiddenu`), and `hiddenu` shows the shell restarting at order 25 again after order 77 — so the two `255`s at 78 and 79 are pre-empted |
| MUSIC1's one `255` (order 99) | **not reached**: `second3` stops at ord 48, and `second3d`, which reaches frame 17,799 — 14,900 frames further on — crosses the `254` at 97 into ord 98 at frame 17109 and holds 98 to the end of its window; order 99 is past every filed window |

So the two questions the plan asks have one answer each, and the second is a negative:
**`254` is stepped over, and the reference never plays a `255` at all** — the shell's own
restarts (`MAIN/U2.ASM:875-905`: orders 18, 25 and 70) land the player past every one of
them. What the player *would* do at a `255` is therefore **not measured by this task**, and
Task 3 must say so in `music.md` rather than read it off stock ST3 semantics.

## 11. Tests

| what | where |
|---|---|
| the census, counted by the test itself, and the 278 records | `crates/sr-assets/tests/omf_symbols.rs` |
| the seven position words as contiguous two-byte slots, the frame/tempo/speed offsets, and the object's own tempo/master-volume initialisers | same |
| the needles: 32 bytes, different records, one occurrence in the object | `xtask/src/music.rs`'s own tests |
| the locator: a unique match, a second one refused, a needle across a 64 KiB boundary, a short needle | same |
| the window (`A..B` half-open), the row round trip, the coverage's gaps and its boundary run | same |
| a window that is back to front, and one whose span overflows a `u64`: findings, never crashes | same + `xtask/tests/music.rs` |
| `music symbols` prints the map | `xtask/tests/music.rs` |
| the trace refusals (where it may be written, the window, the keys, `--force`) | same |
| the traces as references: rows, columns, the manifest's coverage and skipped frames | same |
| `reference verify --tree reference` passes with the traces in it | same |
| **a flipped byte of a trace fails it** (exit 1, both digests named) | same |
| an unknown `kind` is refused by name, a coverage claim the file does not support is a finding | same |
| `music analyse`'s numbers, `--at 0x105B0E`, and the boundary named | same |
| the locator's views: several bases accepted only when the bytes are one memory | `xtask/src/music.rs`'s own tests |
| the locator on a short last chunk (it used to underflow) | same |
| a window out of a manifest that is back to front does not panic the verifier | same |
| two machine readings that disagree are **both recorded**, and a word above 32767 is printed both ways (`a_machine_word_above_32767_is_printed_both_ways`) | same |
| the `zplus` rule against both modules' order lists, restart case included | same |
| **the anchor is per recipe** — 408 for the restart recipes, 399.6 for `SECOND 1` — the two readings within each recipe's measured bound (0.9…2.9 frames for the restart traces, 5.4 for `second1`), and a window trace's extrapolation is not treated as one | `xtask/tests/music.rs` |
| the machine's words recorded twice when they move, once when they do not, and printed both ways | `xtask/src/music.rs`'s own tests |
| **the `zplus` rule re-derived over every trace in `reference/music/`, walked from the tree: 0 mismatches each** | `xtask/tests/music.rs` |
