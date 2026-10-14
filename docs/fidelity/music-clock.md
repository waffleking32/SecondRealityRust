# The music clock — what the reference's own player state says

Phase 3 has no audio reference: the emulator runs silent (D17 requires it, because a real
audio device makes the animation drift), and the two modules' *timing* is invisible from the
outside. What is visible is the player's own state. `MAIN/STMIK.H:125-131` publishes the words
STMIK keeps its position in, `MAIN/STMIK.300`'s OMF `PUBDEF` records give their offsets, and
`dosbox-automation` can read guest memory — so the oracle this phase builds is a **per-frame
trace of the original's own player state**, and Tasks 3, 6 and 8 are measured against it.

**Every number in this note is produced by a command named beside it** (D25), and the three
traces are committed under `reference/music/`:

| trace | command | what it is for |
|---|---|---|
| `reference/music/second5` | `cargo xtask music trace --arg 5 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..1300 --out reference/music/second5` | the phase's oracle: the boot, the music's start, the first graphics frame, ENDLOGO's gate window |
| `reference/music/second3` | `cargo xtask music trace --arg 3 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..2900 --out out/b3` | MUSIC1: its order list, and one of its `254`s |
| `reference/music/hiddenu` | `cargo xtask music trace --arg U --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 --frames 0..6500 --out out/u` | the longest rate window, and MUSIC0's orders 70…77 |
| `reference/music/second5b` | the same command as `second5`, into a fresh directory | **a second session of the same recipe**: the anchor's reproducibility (§2), and the machine's own words moving into their playing values (§5) |

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

**The port's music boot anchor is frame 408**, measured two ways in four sessions.
`music analyse` derives it from the trace's own columns and prints both readings — the
`second5` session:

```
  the anchor the rows imply: the first row change is at frame 427 with `_np_zframe` 13, and
    13 ticks at the measured 1.4622 frames a tick is 19.0 frames — so that row began at
    frame 408.0 (within one tick)
  the anchor the `zframe` column shows: frame 410 is the first frame whose `zframe` is not 0 —
    2.0 frame(s), or 1.4 tick(s), from the rows' reading: the same tick, seen at the next
    whole-frame sample
```

The derivation uses no simulation and no free parameter: the first row change is at frame
427 with the *tick counter's own value* (13) there, and back-extrapolating 13 ticks at the
tick clock's measured 1.4622 frames a tick puts the first tick at **408.0**. (A row change is
seen on a frame, so both the frame and the tick label are quantised — that is the ±1 tick.)

That reading is the same in every session this phase took:

| trace | session | the rows imply | `zframe`'s first non-zero frame |
|---|---|---|---|
| `second5` | 1 | **408.0** | 410 |
| `second5b` | 2 (the same recipe) | **408.0** | 410 |
| `second3` | 3 | **408.1** | 411 |
| `hiddenu` | 4 | **408.1** | 409 |

**So the anchor is a property of the boot and not of a session** — the spread is 0.1 frames —
and the `zframe` column's own reading (409/410/411) is the same tick seen at the *next*
whole-frame sample, which is why it moves by a frame between sessions while the row-derived
value does not. The recipe's anchor is **408**; the note no longer hedges it, and §7's table
is why the traces disagree about the frame the tick is first *visible* on.

Everything is still before it: `ord`, `row` and `pat` are 0, `zframe` is 0, and `speed` 6 /
`tempo` 125 are the object's own initial values (`cargo xtask music symbols` prints the
object; `np_tempo`'s static byte is 0x7D = 125, ST3's default —
`crates/sr-assets/tests/omf_symbols.rs`'s `the_objects_own_initialisers_are_the_notes_values`). The first *row* advances at
frame 427, when the shell has restarted the song at order 25 (`MAIN/U2.ASM:902-905` is the
order-25 restart, `mov bx,25` at `:903`; `restartmus(0, 25, 45000)`).

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
| `second5` | frames 427…961, speed 6 tempo 120 | 61 | 534 | 534.4 | **−0.4** | 8.76 |
| `second3` | frames 427…2472, speed 6 tempo 130 | 253 | 2045 | 2046.0 | **−1.0** | 8.09 |
| `second3` | frames 2488…2896, speed 3 tempo 130 | 101 | 408 | 408.4 | **−0.4** | 4.04 |

The longest window available — 296 row intervals over 3455 frames, 49 seconds of music — is
reproduced to **−2.6 frames, 0.08 %**, and **every steady run's residual is under one row**.
The two runs that are not steady are the two that follow a module-driven speed change, and
they are the third bullet below: `second5` frames 1024…1295 (−35.6, one row 8.76) and
`hiddenu` frames 6182…6495 (−37.4). The rate is the module's own rule and nothing else.

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
analyse` prints them with their positions:

```
  frames 427..961 at speed 6 tempo 120: 61 gap(s) — 8 frame(s) x16, 9 frame(s) x44, 10 frame(s) x1
    the 1 gap(s) outside the 8..9 the run's own row length (8.76 frames) brackets, by their
      position in the run: 18 of 61 (575 -> 585, 10)
  frames 2488..2896 at speed 3 tempo 130: 101 gap(s) — 3 frame(s) x1, 4 frame(s) x95, 5 frame(s) x5
    the 1 gap(s) outside the 4..5 the run's own row length (4.04 frames) brackets, by their
      position in the run: 48 of 101 (2678 -> 2681, 3)
```

`second5`'s 10-frame gap is the 18th of that run's 61 intervals, and `second3`'s 3-frame gap
the 48th of 101: both about a third to a half of the way in, with ordinary intervals on
either side. **Why they happen is not measured here** — this phase does not have an
explanation for them, and Task 3 must not read §4 as "only the ends are special": the trace's
own row-change frames are the acceptance test, not any rule stated over them. (The one run
whose odd gaps *are* at its start is `second5`'s third, frames 1024…1295, where the six
outliers sit at positions 1…5 and 27 — that run is §3's post-speed-change transient, and its
shape there is the transient's.) That is what Task 3's player has to reproduce
tick for tick.

## 5. Which soundcard and mixer rate the reference runs at

`cargo xtask music analyse`, section 4, records each word **twice** — read when the segment
was located (`second5`: frame 147, before the music) and again at the end of the trace:

| word | at the locate | when the music plays | what it is |
|---|---|---|---|
| `_zoutputmode` | **5** | 5 | Sound Blaster Pro (`MAIN/STMIK.H:67-74`) |
| `_zmixspeed` | 20000 | **40000** | the mixing rate: 20 kHz before the demo's music engine starts, **40 kHz** from the music's start on (`cargo xtask music probe --arg 5 --cycles 30000 --frames 408..1300 --at 0x5AC5` reads 40000 at both ends of that window) |
| `_zbufsize` | **4096** | 4096 | the DMA buffer, the largest `MAIN/STMIK.H:88-95` lists |
| `_zpollmix` | 0 | **1** | **the caller polls the player** (`MAIN/STMIK.H:152-176`): `1` means the program must call `zpollme`, which is what `MAIN/COPPER.ASM:466` does once per copper frame — and the same probe reads 1 at both ends of the music window |
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
`machine` and `machine_moved` — `second5b`'s manifest carries both — because which words move
is not a rule this tool gets to assume: the mixing rate moves, `zpollmix` moves, and the
volume words move.

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
rule it computes is the one that reproduces every sample of all three filed traces.
`cargo xtask music analyse` re-derives it from each trace's own order sequence and prints the
count of samples it had to be right about — **0 mismatch(es) over 10,256 samples** (1,150 +
2,755 + 6,351):

```
  the port's zplus rule, re-derived from this trace's own order sequence: 2755 sample(s)
    checked, 0 mismatch(es)
    rule: 1 while playing an order whose next entry in the module's order list is 254; 2 on a
      crossing (the counter moved from o to o+2 with orders[o+1] == 254) unless the entry
      after the one entered is itself 254, in which case 1; 0 otherwise; a seek or a restart
      credits no marker behind it
    the orders the counter held, in order: [0, 42, 43, 44, 45, 47, 48]
```

So, **as the port's rule**:

- **1** while playing an order whose next entry in the module's order list is `254`;
- **2** on a marker crossing — the counter moved from `o` to `o + 2` with `orders[o + 1] ==
  254` — **unless** the entry after the one entered is itself `254`, in which case **1**
  (this is MUSIC1's `2 → 1` at order 48, whose successor at order 49 is a `254`);
- **0** otherwise;
- and a seek or a restart **credits no marker behind it**: a crossing needs the order the
  counter came from to be the one before the skipped entry, which is why `SECOND 3`'s restart
  at order 42 — whose list predecessor is a `254` the player never played — reads 0.

**What is extrapolation, said plainly.** One case the rule distinguishes is measured
nowhere: an order two steps past a marker **whose own successor is not a marker**, where the
value should fall from the crossing's `2` back to `0`. `MUSIC0`'s crossing is measured
(`second5` and `hiddenu` both hold order 28) but neither holds order 29; `MUSIC1`'s crossing
is measured too (`second3` holds order 47) and it holds order **48**, two past the marker at
46 — but that order reads **1**, not `0`, because the entry after it (order 49) is itself a
`254`, so the rule's "unless the next entry is also a `254`" case is what it tests. So **`2`
returning to `0` on the next natural advance is not measured**; it is what the rule says and
what the three traces' surviving cases are consistent with, and the first trace that plays
past a marker without another one behind it will test it. Nor does any trace play a `255`
(§10), so nothing here says what `zplus` does there.

`muscode_6` (`DIS/DISINT.ASM:242-282`) shows how the *parts* use the value — a row offset
computed from `zplus` 0/1/2 for `dis_muscode` — and that is a reading of the source, not a
measurement of this rule; the rule above is a measurement of the traces.

`zinfo` is **0 in all three traces** — 10,256 samples over the 1,300 + 6,500 + 2,900-frame
windows, covering MUSIC0's orders 0 and 25…28 and 70…77 and MUSIC1's orders 0 and 42…48.
**These captures therefore do not measure what its other values are**, and `D8`'s ladder is
answered only in its first step: route 1 (read the object's symbols, then the words live) was
performed and produced a map and a trace, and the word it watches stayed 0. `D8` now records
the outcome and the substitute the port uses; this note does not guess at a rule the traces do
not show.

## 7. What a trace does not hold, and why it moves between sessions

Every trace holds a run of frames and misses exactly three — a boundary run of one or two
frames and one frame in the middle — and `music analyse` names the boundary run:

```
  frames 148..1299 of the requested 147..1299: 1150 sample(s), 3 frame(s) missing
    (147, 409, 1220) (frames 147..147 of them are the boundary: the segment was located
    after they had gone) — this trace is NOT per-frame (recomputed from the trace file's
    own frames)
```

| trace | sampled | not in it |
|---|---|---|
| `second5` | 148…1299 | 147 (the boundary — the frame the segment was located on: locating costs frames, and that one was already gone), **409**, 1220 |
| `second3` | 144…2899 | 142…143 (the boundary), **417** |
| `hiddenu` | 148…6499 | 146…147 (the boundary), **410** |
| `second5b` | 146…1299 | 144…145 (the boundary), **409**, 1220 |

Each middle gap is a frame the guest's own counter moved across without the trace being able
to read it, and the manifest carries what the counter did around it: for `second5`, `skipped`
records `409..409 (the counter went 408 -> 410)` and `1220..1220 (the read that was opened on
frame 1220 was still open when the guest moved to 1221)`. **Which index falls in the gap is
session-dependent** — the traces lose 409 (`second5`, `second5b`), 417 (`second3`) and 410
(`hiddenu`) — and all of them sit at the same point in the demo, the boot's first graphics
frame (the reference's own captures put that at 404, D17).

**Why is a hypothesis.** What is measured is the counter's own readings, and they say only
that it went from one value to another with the frame between never current. The explanation
— that a video-mode change publishes no frame for a moment, so the emulator's counter steps
over one (D17 measured the same behaviour: "a video-mode stop or reset calls `GFX_EndUpdate`
without publishing") — is consistent with all three traces and with where they all sit, and
is **not** measured here. `second5`'s 1220 has the same shape at the point the reference's own
index table puts the ENDLOGO → CRED change in (1180, 1440]
(`reference/parts/endlogo/manifest.json`, `reference/parts/cred/manifest.json`, both
`--arg 5`), and is not measured either.

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

Three traces, two tempos, and the tick rate each time: `_np_zframe` is the tick clock, and
its rate is a second, independent way of measuring what §3 measures through the row rate.

`docs/fidelity/reference-indices.md` §1.7 records a different word: a 16-bit word at physical
address **`0x105B0E`** advancing **0.7424 times a video frame**, reading 371/373/374 at video
frame 900. `cargo xtask music probe --arg 2 --cycles 30000 --frames 899..3000 --at 0x105B0E`
measures it — **and `_np_zframe` beside it, in the same bracketed samples**, so the two are
comparable frame for frame — and run twice in two sessions it gives the same answer:

```
address 0x105B0E: the segment's base(s) are 0x0051F0, 0x1051F0 (of 29607 byte(s))
  in the view at 0x1051F0 it is that base + 0x091E
  the last published symbol at or below it is _zmixercount (base + 0x091C, 2 byte(s) below);
    the next above is _zchn at base + 0x0DA0
  frame 900: 370; frame 2999: 1927 — +0.7418 per frame, +51.99 per second at 70.086 Hz
  2099 sample(s) between them
  _np_zframe over the same window: 1 at frame 1112 to 1401 at frame 2999 — +0.7419 per frame,
    +52.00 per second (1 reset(s))
  the player's own words at frame 2999: tempo 130, speed 3, zframe 1401, ord 10, row 63
```

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

**The three filed manifests are older than the tool that reads them, in two ways, and they
are left as they were.** They carry no `coverage.before_sampling` (§7: the field that names
the boundary run) and their `locate.aliasing` sentence still contains a doubled `0x` that the
tool no longer writes. Neither is repaired here: `reference/` is input-only, a *new* directory
under `reference/music/` is the only thing `music trace` may write inside it, and hand-editing
a filed manifest is what Phase 2's `reference backfill` exists to avoid being done by a
person. What keeps the filed records checkable is the reading side: `reference verify`
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
| MUSIC1's one `255` (order 99) | **not reached** in the 2,900-frame capture, which stops at ord 48 |

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
| two machine readings that disagree are refused, not silently merged | same |
| the `zplus` rule against both modules' order lists, restart case included | same |
| **the anchor is 408 in all four filed sessions**, and the two readings are within one tick | `xtask/tests/music.rs` |
| the machine's words recorded twice when they move, once when they do not | `xtask/src/music.rs`'s own tests |
| **the `zplus` rule re-derived over all three filed traces: 0 mismatches, 10,256 samples** | `xtask/tests/music.rs` |
