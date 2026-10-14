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
```

**The port's music boot anchor is frame 409** — the frame between the last sample that shows
`zframe 0` (408) and the first that shows it advancing (410) — and the trace does not hold
frame 409 itself (see §7). The same analysis on `hiddenu` shows the same thing with 409 in
the trace: `first advancing frame: 409`, `zframe 0 -> 1`.

Everything is still before it: `ord`, `row` and `pat` are 0, `zframe` is 0, and `speed` 6 /
`tempo` 125 are the object's own initial values (`cargo xtask music symbols` prints the
object; `np_tempo`'s static byte is 0x7D = 125, ST3's default). The first *row* advances at
frame 427, when the shell has restarted the song at order 25 (`MAIN/U2.ASM:890-894`,
`restartmus(0, 25, 45000)`).

## 3. The tick rate, and the rule behind it

A tick is `2.5/tempo` seconds and a row lasts `speed` ticks, so the row rate is
`speed × tempo/2.5` rows a second — **a prediction from the module's own header and from the
player's own speed and tempo words, not a fit to the trace**. MUSIC0's header carries speed 6
and tempo 125, and `MAIN/STARTMUS.C:257` patches its tempo byte to 0x78 = 120 before playing
it; MUSIC1's carries speed 3 and tempo 130.

`cargo xtask music analyse` measures each run of constant speed and tempo separately (the
modules change both as they play, `Axx`/`Txx` cells), between the run's first and last row
change so that the part-row at each end does not enter the residual:

| trace | run | row intervals | frames | predicted | residual | one row |
|---|---|---|---|---|---|---|
| `hiddenu` | frames 430…3885, speed 8 tempo 120 | 296 | 3455 | 3457.6 | **−2.6** | 11.68 |
| `hiddenu` | frames 3897…6119, speed 6 tempo 120 | 254 | 2222 | 2225.2 | **−3.2** | 8.76 |
| `second5` | frames 427…961, speed 6 tempo 120 | 61 | 534 | 534.4 | **−0.4** | 8.76 |
| `second3` | frames 427…992, speed 6 tempo 130 | 70 | 565 | 566.1 | **−1.1** | 8.09 |

The longest window available — 296 row intervals over 3455 frames, 49 seconds of music — is
reproduced to **−2.6 frames, 0.08 %**, and the residual is under one row (11.7 frames) in
every run. The rate is the module's own rule and nothing else.

**Two things the trace says that the files do not.**

- **The player's speed word, not the module's header byte, is what the row rate follows.**
  `second3` is MUSIC1 — its `(ord, pat)` pairs match `assets/MAIN/MUSIC1.S3M`'s order list at
  every order the counter held (`analyse` identifies the module that way, not by speed) — and
  the trace's own `_np_speed` reads **6** while that file's header byte 0x31 is **3**. The
  measured row interval is 8.09 frames: speed 6 at tempo 130, where speed 3 would be 4.04.
  What writes 6 is not measured here; the *rate* is, and it is the trace's word that predicts
  it.
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
| `hiddenu` | speed 8 tempo 120 | 11 frames ×97, 12 frames ×199 |
| `hiddenu` | speed 6 tempo 120 | 8 ×64, 9 ×190 |
| `second5` | speed 6 tempo 120 | 8 ×16, 9 ×44, 10 ×1 |
| `second5` | speed 1 tempo 120 | 1 ×16, 2 ×13 |
| `second3` | speed 6 tempo 130 | 8 ×44, 9 ×26 |

Two gap sizes per run is the whole of it (a third appears once, at the transient of §3): the
row rate is **periodic on the frame clock with a two-frame period**, and the frames the rows
land on are reproducible from the run's start. That is what Task 3's player has to reproduce
tick for tick.

## 5. Which soundcard and mixer rate the reference runs at

`cargo xtask music analyse`, section 4 — read from the guest at the frame the segment was
located (`second5`: frame 147) and again after the re-locate (frame 1422):

| word | value | what it is |
|---|---|---|
| `_zoutputmode` | **5** | Sound Blaster Pro (`MAIN/STMIK.H:67-74`) |
| `_zmixspeed` | **20000** | the mixing rate, 20 kHz |
| `_zbufsize` | **4096** | the DMA buffer, the largest `MAIN/STMIK.H:88-95` lists |
| `_zpollmix` | **0** | the player mixes from its own timer interrupt, not from `zpollme` |
| `_zspeedadjust` | **0** | |
| `_zspeedadjust2` | **0** | `MAIN/U2.ASM:389` writes it only for outputmode 7 or 2, and the card is 5 |
| `_np_mastervol` | **192** | the module's own global volume (0xC0) |
| `_np_mastermul` | **0** | |

**This settles spec §7.2's default from the machine rather than from `MENU.C`'s menu order:
one of the four rates the shell offers is 20000 Hz (`MAIN/U2.ASM:737-743`), and that is what
the reference holds** — not 40000, and not a rate derived from the card. And
`_zpollmix = 0` says the reference is *not* the "call `zpollme` once a frame" mode: mixing
runs from the player's own timer, which is why `_np_zframe`'s rate (§8) is the tick rate and
not the frame rate.

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

**What the value counts is not measured by these traces** — the increments sit either side of
a `254` but they do not count them (MUSIC1's goes 0→1→2→**1**), and `muscode_6`
(`DIS/DISINT.ASM:242-282`) only shows how the *parts* use it: a row offset computed from
`zplus` 0/1/2 for `dis_muscode`. That is a hypothesis and is recorded as one.

`zinfo` is **0 in all three traces** — 10,256 samples over the 1,300 + 6,500 + 2,900-frame
windows, covering MUSIC0's orders 0 and 25…28 and 70…77 and MUSIC1's orders 0 and 42…48.
**This capture therefore does not measure what its other values are**, and `D8`'s ladder is
answered only in its first step: route 1 (read the object's symbols, then the words live) was
performed and produced a map and a trace, and the word it watches stayed 0. `D8` records the
substitute cue for GRID, HARD and the vault; this note does not guess at a rule the trace
does not show.

## 7. What the trace does not hold

`second5` holds every frame from 148 to 1299 except three, and the manifest's `coverage` says
so (`"per_frame": false`):

| frame(s) | what the guest's own counter did |
|---|---|
| 147 | the frame the segment was located on: locating costs frames, and this one was already gone |
| 409 | the counter was read as 408 and then as 410 with no reading of 409 between: the emulator published no such frame (it is the boot's first graphics frame — the reference's own captures put the demo's first graphics frame at 404, D17) |
| 1220 | the read opened on frame 1220 was still open when the guest moved to 1221, so its bytes belong to neither frame with certainty and were dropped; the reference's own index table puts the ENDLOGO → CRED change in (1180, 1440] (`reference/parts/endlogo/manifest.json`, `reference/parts/cred/manifest.json`, both `--arg 5`) |

The tool does not call such a trace per-frame, and a trace that quietly sampled every other
frame is refused the same description — the check is recomputed from the trace's own frames
by `reference verify` rather than believed.

## 8. `_np_zframe`, and the word `reference-indices.md` §1.7 measured

`_np_zframe` advances one per **tick**, not per frame. `cargo xtask music analyse --trace
reference/music/second5`, section 2, prints it as a rate of its own: it goes from 1 at frame
410 to 609 at frame 1299 — **608 ticks in 889 frames, 0.6839 a frame, 47.93 a second** — where tempo 120's rule gives 48.00. It is the word the parts set and wait on
(`DIS/DISC.ASM:138-159`, `DIS/DISINT.ASM:412-425`), and it starts from 0 when the music does
(0 at frame 408, 1 at 410).

`docs/fidelity/reference-indices.md` §1.7 records a different word: a 16-bit word at physical
address **`0x105B0E`** advancing **0.7424 times a video frame**, reading 371/373/374 at video
frame 900. Two commands place it:

`cargo xtask music probe --arg 2 --cycles 30000 --frames 899..3000 --at 0x105B0E`, run twice
in two sessions, prints the same answer both times:

```
address 0x105B0E: the segment's base(s) are 0x0051F0, 0x1051F0 (of 29607 byte(s))
  in the view at 0x1051F0 it is that base + 0x091E
  the last published symbol at or below it is _zmixercount (base + 0x091C, 2 byte(s) below);
    the next above is _zchn at base + 0x0DA0
  frame 900: 370; frame 2999: 1927 — +0.7418 per frame, +51.99 per second at 70.086 Hz
```

`cargo xtask music analyse --trace reference/music/second5 --at 0x105B0E` places the same
address against the *filed* trace's symbols.

So:

- **It is not `_np_zframe`**, which is at base + `0x08FB`. Offset `0x091E` is an **unnamed
  word of the player's own data area**, two bytes past `_zmixercount` (base + `0x091C`, whose
  two-byte word ends at `0x091D`), and some thousand bytes below the next published symbol,
  `_zchn`.
- **The rate reproduces.** Two sessions of this task's own probe give **+0.7418 per frame**
  (51.99 a second at the demo's 70.086 Hz) against §1.7's 0.742016/0.742012/0.741909 — the
  same counter, at the same base, within **3 parts in ten thousand**.
- **The value at frame 900 reproduces**: this probe reads **370** where §1.7's three sessions
  read 371/373/374 — a spread of four ticks, which is the one-to-two frames §1.7 itself
  measured between sessions (D17).

So §1.7's address is right and its name is not: the word is a player counter at **52.0 a
second**, which is the tick rate of tempo **130** — not the rate of the tempo the module in
the same window plays at (120, whose ticks `_np_zframe` counts at 47.93 a second — the very
next table). What drives it is **not measured here**, and this note does not guess. **The
phase's clock is `_np_zframe`**, the word the parts set and wait on and the one `np_row` and
`np_ord` travel with.

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

## 10. The order list: what the player does at `254` and `255`

`cargo xtask music analyse`, section 6, reads this off the order counter's own sequence:

| entry | what the trace shows |
|---|---|
| MUSIC0 `254` at order 27 | **stepped over**: the counter went ord 26 → ord 28 without resting on 27, having played each of them (`second5`, frame 1260; the same in `hiddenu`) |
| MUSIC1 `254` at order 46 | **stepped over**: ord 45 → ord 47 without resting on 46 (`second3`, frame 2488) |
| MUSIC0's six `255`s (orders 16, 17, 64, 69, 78, 79) | **not played**: the shell's restarts jump over 16 and 17 (ord 0 → ord 25 at frame 427) and order 70 (ord 28 → 70 in `hiddenu`), and `hiddenu` shows the shell restarting at order 25 again after order 77 — so the two `255`s at 78 and 79 are pre-empted |
| MUSIC1's one `255` (order 99) | **not reached** in a 2,900-frame capture that stops at ord 48 |

So the two questions the plan asks have one answer each, and the second is a negative:
**`254` is stepped over, and the reference never plays a `255` at all** — the shell's own
restarts (`MAIN/U2.ASM:875-905`: orders 18, 25 and 70) land the player past every one of
them. What the player *would* do at a `255` is therefore **not measured by this task**, and
Task 3 must say so in `music.md` rather than read it off stock ST3 semantics.

## 11. Tests

| what | where |
|---|---|
| the census, counted by the test itself, and the 278 records | `crates/sr-assets/tests/omf_symbols.rs` |
| the seven position words as contiguous two-byte slots, and the frame/tempo/speed offsets | same |
| the needles: 32 bytes, different records, one occurrence in the object | `xtask/src/music.rs`'s own tests |
| the locator: a unique match, a second one refused, a needle across a 64 KiB boundary, a short needle | same |
| the window (`A..B` half-open), the row round trip, the coverage's gaps | same |
| `music symbols` prints the map | `xtask/tests/music.rs` |
| the trace refusals (where it may be written, the window, the keys, `--force`) | same |
| the traces as references: rows, columns, the manifest's coverage and skipped frames | same |
| `reference verify --tree reference` passes with the traces in it | same |
| **a flipped byte of a trace fails it** (exit 1, both digests named) | same |
| an unknown `kind` is refused by name, a coverage claim the file does not support is a finding | same |
| `music analyse`'s numbers, and `--at 0x105B0E` | same |
| the locator's views: several bases accepted only when the bytes are one memory | `xtask/src/music.rs`'s own tests |
