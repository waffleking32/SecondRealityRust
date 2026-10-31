# The music clock — what the reference's own player state says

The emulator runs silent (D17: a real audio device makes the animation drift), so the modules'
timing is invisible from outside. What is visible is the player's own state: `MAIN/STMIK.H:125-131`
publishes the words STMIK keeps its position in, `MAIN/STMIK.300`'s OMF `PUBDEF` records give their
offsets, and `dosbox-automation` reads guest memory. The oracle is therefore a **per-frame trace of
the original's own player state**, filed under `reference/music/`. This note owns what the traces
show; how the player works, read from the object, is `music.md`'s.

Every trace was taken with one recipe (D17), which its manifest records:

```
cargo xtask music trace --arg <arg> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <window> --out reference/music/<trace>
```

| trace | arg | frames | what it holds |
|---|---|---|---|
| `second5` | 5 | `0..1300` | the boot, the music's start, ENDLOGO's gate window |
| `second5b` | 5 | `0..1300` | a second session of `second5`'s recipe (§2, §5) |
| `second3` | 3 | `0..2900` | MUSIC1's order list and one of its `254`s |
| `hiddenu` | U | `0..6500` | MUSIC0's orders 70…77, recipe `U`'s anchor |
| `second1` | 1 | `148..6000` | `SECOND 1`'s boot: ALKU's cue window and the block-1 anchor |
| `second1b` | 1 | `7600..7960` | PAM's `dis_sync()<10` wait and its 40 drawn frames |
| `second3b` | 3 | `10200..15050` | DOTS's and RAYSCRL's `dis_musplus` cue windows |
| `hiddenu2` | U | `6500..8200` | DDSTARS's 8000 keyframe and the stage marks around it |
| `second2` | 2 | `4700..9300` | TECHNO's and PANICEND's start windows |
| `second3a` | 3 | `2900..10200` | LENS's and PLZPART's start windows; joins `second3` to `second3b` |
| `second3d` | 3 | `15000..17800` | COMAN's and JPLOGO's start windows |
| `second1c` | 1 | `5900..7600` | U2A's gate 1 and gate 2 (§2.1) |
| `second4` | 4 | `500..4400` | the block-4 boot's MUSIC0@18 restart and U2E's order-19 gate (§2.1) |
| `second3e` | 3 | `18000..22000` | the mid-run MUSIC0@18 restart and U2E's opening (§2.1) |
| `second2b` | 2 | `380..3800` | `SECOND 2`'s boot: BEGLOGO→GLENZ, GLENZ's music-wait exit and intro (§2.2) |
| `second1d` | 1 | `8100..8800` | the block-1 `MUSIC1@0` restart's session and GLENZ's block-1 wait exit (§2.2) |
| `second1e` | 1 | `8800..33950` | block 1 from 8800 to JPLOGO, PANICEND's cue wait (§2.3) |
| `second1f` | 1 | `34105..48277` | block 1 from the U2E restart + 50 to ENDSCRL's first frame + 2000 (§2.3) |

A trace cannot follow a reload of the player's segment (§1), so windows stop short of a restart.
`second3d` ends at 17800 because block 3's `MAIN/U2.ASM:880-885` MUSIC0@18 restart reloads STMIK
in (17847, 18000): a window to 18000 is refused with `_np_ord reads -1, which is not a byte value:
the base 0x0051F0 is not this segment's`. `second1e` ends at 33950 because the U2E restart's reload
is already under way by the end-of-trace re-scan after 34004 (`8800..34070` refuses the same way,
and `8800..34005` refuses because the two views of the segment differ, §9).

The symbol map, and the analysis every section below quotes:

```
cargo xtask music symbols --object upstream/SecondReality/MAIN/STMIK.300
for t in reference/music/*/; do cargo xtask music analyse --trace "$t"; done
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

`cargo xtask music symbols` prints these and the other 36 published symbols, the record census
(**278 records**: THEADR 1, COMENT 5, MODEND 1, **PUBDEF 45**, LNAMES 2, **SEGDEF 1**, LEDATA 105,
LIDATA 118), the one segment (`text_stmik300_text`, class `CODE`, **29,607 bytes**, `acbp` 0x68)
and the two needles. The map is D8's route 1, the object's own symbols. The seven position words
are one contiguous run of two-byte slots (`0x08F7` … `0x0903`), which
`crates/sr-assets/tests/omf_symbols.rs` asserts slot by slot because `MAIN/STMIK.H:125-131`'s
declaration order is not the memory order.

The trace guarantees three things:

- **It is keyed by the guest's own frame counter.** Each sample is bracketed by two reads of the
  counter and dropped when the counter moved between them: a late sample is a missing frame, never
  a wrong value under the right number.
- **One memory read per sample, of the whole segment**, so nothing compared frame by frame can
  tear.
- **The segment is located by the object's own bytes, twice.** Two 32-byte runs with no repeated
  byte, from different `LEDATA` records, must solve the same base; the scan is re-run at the end of
  every trace and the manifest records both.

## 2. Where the music starts

`music analyse`, section 2, derives the anchor two ways from the trace's own columns and prints
both, one line each. On `second5` (whose `zframe` reads 0 at 408 and 1 at 410):

```
  the anchor the rows imply: the first row change is at frame 427 with `_np_zframe` 13, and 13 ticks at the measured 1.4622 frames a tick is 19.0 frames — so that row began at frame 408.0 (± the tick the row boundary is within and the frame the change is seen on)
  the anchor the `zframe` column shows: frame 410 is the first frame whose `zframe` is not 0 — 2.0 frame(s), or 1.4 tick(s), from the rows' reading: the same tick, seen at the next whole-frame sample. Both readings are quantised to a whole frame, so the gap is one frame plus the tick the row change is seen within; it is not a claim that they agree to within a tick, and the two numbers above say how big it is.
```

The row reading back-extrapolates the tick counter's value at the first row change at the
measured tick rate: no simulation and no free parameter.

| trace | frames | module | the rows imply | `zframe`'s first non-zero frame |
|---|---|---|---|---|
| `second5` | 148..1299 | MUSIC0 | **408.0** | 410 |
| `second5b` | 146..1299 | MUSIC0 | **408.0** | 410 |
| `second3` | 144..2899 | MUSIC1 | **408.1** | 411 |
| `hiddenu` | 148..6499 | MUSIC0 | **408.1** | 409 |
| `second1` | 148..5999 | MUSIC0 | **399.6** | 405 |
| `second2b` | 380..3799 | MUSIC1 | **401.1** | 408 |
| `second4` | 500..4399 | MUSIC0 | **408.2** — the block-4 boot's own MUSIC0@18 restart (§2.1) | 500 |
| `second1b` | 7600..7959 | MUSIC0 | *371.9* | 7600 |
| `second3b` | 10200..15049 | MUSIC1 | *5727.6* | 10200 |
| `hiddenu2` | 6500..8199 | MUSIC0 | *390.7* | 6500 |
| `second2` | 4700..9299 | MUSIC1 | *1108.9* | 4700 |
| `second3a` | 2900..10199 | MUSIC1 | *2450.8* | 2900 |
| `second3d` | 15000..17799 | MUSIC1 | *5723.6* | 15000 |
| `second1c` | 5900..7599 | MUSIC0 | *399.7* | 5900 |
| `second3e` | 18000..21999 | MUSIC0 | *4718.9* | 18000 |
| `second1d` | 8100..8799 | MUSIC1 | *997.0* | 8100 |
| `second1e` | 8800..33949 | MUSIC1 | *8523.4* | 8800 |
| `second1f` | 34105..48276 | MUSIC0 | *20927.5* | 34105 |

Frames are each manifest's coverage, and the module is the one `music analyse` identifies from the
`(ord, pat)` pairs. Italic rows are windows: their "anchor" is an extrapolation, not a start.

**The anchor is a property of the boot recipe.** The restart recipes (`SECOND 3`, `5`, `U`) agree
on **408**. `SECOND 1` derives **399.6**: block 1 never takes a block restart (its row of
`MAIN/U2.ASM:766-775` loads with `bx = -1`, "no seek"), and its play is the initial `_zplaysong(0)`
at `:785-787`, while the restart recipes take `restartmus`/`_zplaysong` at their starts (`:856-863`,
`:898-905`, `:889-894`). `SECOND 2` derives **401.1**: its start is `restartmus(1, 0, 48700)`
(`:766-775`) then `_zplaysong(0)` (`:785-787`), MUSIC1 at order 0, and its two readings straddle
both other recipes' (408 by `zframe`, near 399.6 by the rows), with the boot's first ticks seen
nearly one a frame (`zframe` 0 → 2 between 407 and 408, 2 → 14 by 420). `second1`'s 5.4-frame gap
between its two readings is a sample at 405 that caught two ticks at once. `second4`'s 408.2 is a
real start: the block-4 boot's first part is U2E, so the `:880-885` restart fires at the first
graphics frame and the whole window is that player's session.
`xtask/tests/music.rs`'s `the_anchor_is_the_same_in_every_filed_session` asserts the per-recipe
anchors, walking `reference/music/`, and refuses to treat a window's extrapolation as one.

**A restart does not zero `_np_zframe`.** A recipe's first play starts from the static zero, which
is why every boot trace reads 0 at its anchor; `second3e` reads 9,099 at 18000, carried from
PLZPART's `_dis_setmframe(0)` at 5,723 through the U2E restart (§2.1).

**What the windows' extrapolations are.** Each back-dates the last tick-counter reset before its
window, at the rate measured over its own frames. `second3b`'s 5727.6 and `second3d`'s 5723.6 are
PLZPART's `_dis_setmframe(0)` (`PLZPART/PLZ.C:69`, held by `second3a` at 5723); `second3a`'s 2450.8
is LNS's (`LENS/MAIN.C:106`, held by `second3` at 2452); `second2`'s 1108.9 and `second1e`'s 8523.4
are GLENZ's resets at 1111 and 8521 (§2.2). `second1c`'s 399.7 is block 1's 399.6, a session with
no reset. `second1b`'s 371.9 and `hiddenu2`'s 390.7 hold no reset either but land 27.7 and 17.3
frames low, short windows' rate errors carried over thousands of ticks. `second3e`'s, `second1d`'s
and `second1f`'s are meaningless: the counter carries across a reload, and the ticks were earned
partly under another module's tempo.

**The windows measure the session phase spread.** `hiddenu` ends at 6499 reading `_np_zframe` 4175
at ord 28 row 9; `hiddenu2` begins at 6500 reading 4185 at ord 28 row 11: two sessions of one
recipe ten ticks (about fifteen frames) apart, D17's host-timing term. `second3`→`second3a` join
across one frame at the next row (`_np_zframe` 331 → 333, a second tick caught in the gap) and
`second3a`→`second3b` at the same row one tick apart (3321 → 3322). `second3d` overlaps `second3b` at 15000..15049 and
crosses the same order boundary (91 → 93) seven frames later (15038 against 15031).

**The initial words.** Before the music starts `ord`, `row`, `pat` and `zframe` are 0, and speed 6
/ tempo 125 are the object's initialisers (`np_tempo`'s static byte is 0x7D;
`the_objects_own_initialisers_are_the_notes_values`). `second5`'s first row advances at 427, under
the shell's order-25 restart (`MAIN/U2.ASM:902-905`, `mov bx,25` at `:903`).

**The port's player against the traces.** `crates/sr-audio/tests/trace.rs`'s
`the_port_reproduces_every_filed_trace` drives a `Player` from each recipe's anchor over every frame
a trace holds (its coverage checked against the manifest, `check_coverage`). It asserts **content**
(for `order`, `pattern`, `row`, `speed`, `tempo`, `loop` and `zplus`, the port's sequence of values
is the trace's) and **placement** (within `JITTER_FRAMES`). A window's run is shifted by the offset
its first row change measures (−16 for `hiddenu2`, +1 for `second1b` and
`second3b`, +2 for `second2` and `second3a`, +4 for `second3d`). No player can match exactly: `second5` and
`second5b` disagree on 8 of their 131 row changes (`the_filed_traces_are_what_the_comparison_assumes`). A12 is the residual; A54 and
`KNOWN_DISAGREEMENTS` hold two block-1 traces (§2.3).

### The order in force at each part's start

`music_for_part` uses the order each part hears when its start window opens. The orders are the
traces' own `ord` column:

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
| PLZPART | (5720, 5730] | `second3a` | MUSIC1 **62** — from frame 5723 (row 0, pattern 43, `_np_zframe` 0, the part's `_dis_setmframe(0)`, `PLZPART/PLZ.C:69`); frame 5720 is still order 60 row 63, the advance over the `254` at 61 landing with the reset |
| TECHNO | 4797 | `second2` | MUSIC1 **18** — frame 4797, row 60, pattern 3; `_np_zframe` reads 0 at frame 4794 (`TECHNO/KOE.C:214`) |
| PANICEND | (8780, 8790] | `second2` | MUSIC1 **37** — rows 53..55, pattern 16; the part has no music call of its own |
| COMAN (`3dsinfld`) | (15000, 15100] | `second3d` | MUSIC1 **91 at the window's opening, 93 from frame 15038**, over the `254` at 92; nothing marks COMAN's first frame, so the value used, 91, is **derived, not measured** |
| JPLOGO | (17000, 17100] | `second3d` | MUSIC1 **96** — rows 50..62, pattern 71; the counter crosses the `254` at 97 to 98 at frame 17109 |

### 2.1 The orders in force at U2A's and U2E's starts, and the U2E restart's clock

**U2A (block 1).** The gate is `order>10 && row>46` (`VISU/C/U2A.C:229-239`):

```
$ awk -F, '$2 == 11 && $3 == 47' reference/music/second1c/trace.csv | head -1
6988,11,47,8,0,6,120,0,0,4513
$ awk -F, '$1 == 7618' reference/music/second1b/trace.csv
7618,12,55,11,0,6,120,0,0,4945
```

Gate 1 passes at **6988** (order 11 row 47). Gate 2's `order>11 && row>54` first holds at **7618**
(order 12 row 55), a **music-only value**: U2A's animation stream ends first (~7510), so the part
normally retires at the stream's end before it. `second1c`'s orders advance at 6016 (10), 6576 (11)
and 7136 (12).

**U2E (block 4's boot; blocks 1–3 after the restart).** The gate is `order>18`
(`VISU/C/U2E.C:358-366`):

```
$ awk -F, '$2 == 19' reference/music/second4/trace.csv | head -1
699,19,0,51,0,6,120,0,0,199
$ awk -F, '$1 == 1000 || $1 == 1600 || $1 == 2600 || $1 == 3600' reference/music/second4/trace.csv
1000,19,34,51,0,6,120,0,0,405
1600,20,38,49,0,6,120,0,0,816
2600,22,25,52,0,6,120,0,0,1501
3600,24,11,47,0,6,120,0,0,2187
```

**Frame 699** in `second4`. At speed 6 a row is 8.76 frames (`music analyse`, section 3, gives
10.22 at speed 7 and 11.68 at 8 for the order-24 excursion) and the orders start every 560–561
frames; order 25 starts at 4131.

**The mid-run restart's clock (`second3e`).** `second3e` opens inside the MUSIC0@18 session the
shell installs at the U2E boundary (`MAIN/U2.ASM:880-885`):

```
$ awk -F, '$1 == 18000 || $1 == 18100' reference/music/second3e/trace.csv
18000,18,16,50,0,6,120,0,0,9099
18100,18,27,50,0,6,120,0,0,9168
$ awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second4/trace.csv
$ awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second3e/trace.csv
```

`second4` publishes 18:16 and 18:27 at 558 and 658, and every order boundary differs by the same
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

So the restarted clock starts at **408.2 + 17,442 = 17,850.2** (±1), and the boundary's first
graphics frame is four frames earlier at 17,846 (the boot's 404→408): D34's four-tick wait. In the
chain's mapping (D31) the boundary is chain 17,427 and the first tick chain 17,431.

`_np_zframe` across the restart: `second3d` reads 8,961 at 17,799; 47 frames at MUSIC1's tempo 130
(~35 ticks), the 4 unpolled wait frames, and 150 frames at MUSIC0's tempo 120 (~103 ticks) give
**8,961 + 35 + 0 + 103 = 9,099**. The restart's phase is carried by the order/row words, which is
what both vector parts' gates read (`int 0fch` fn 6). The screens agree as the music does
(`reference-indices.md` §18.1).

### 2.2 The block-2 and block-1 GLENZ boundaries

**Block 2.** `second2b` is `SECOND 2`'s boot from 380. The port's BEGLOGO→GLENZ boundary is
**576** (BEGLOGO's 172 ticks after 404, `beglogo.md` §3), and the trace reads MUSIC1 there:

```text
$ awk -F, '$1 == 576' reference/music/second2b/trace.csv
576,0,40,73,0,3,130,0,0,129
$ awk -F, '$1 >= 1109 && $1 <= 1112' reference/music/second2b/trace.csv
1109,2,44,50,0,3,130,0,1,525
1110,2,44,50,0,3,130,0,1,526
1111,2,45,50,0,3,130,0,1,0
1112,2,45,50,0,3,130,0,1,1
```

GLENZ's wait (`GLENZ/MAIN.C:290`) is `while(!dis_exit() && dis_musplus()<-19)`. At 576 `zplus` is
0, so the −32 sentinel holds it; the first value that opens it is order 2 row 45 with `zplus` 1,
`musplus` −19, and `dis_setmframe(0)` runs there: **the reset is 1111**, the zoomer's first tick,
and the frame captures first change on 1111 too (`reference-indices.md` §7). The intro's two `getmframe` waits read the same file
(`awk -F, '$1 >= 1513 && $1 <= 1562' reference/music/second2b/trace.csv`): `getmframe()<300` first
sees 300 on 1515 (then `dis_waitb` on 1516 and 1517), `getmframe()<333` first sees 333 on 1560
(then a `dis_waitb` on 1561), and the main loop's first `dis_waitb` is 1562. So main-loop frame f is
the picture at reference **`1562 + f`**.

**Block 1.** The block-1 `restartmus` (`MAIN/U2.ASM:813-818`) reloads the segment, so a window
across it refuses (`7800..8800`: `_np_ord reads -1`). The last pre-reload window, `7800..8050`,
reads MUSIC0 at 8049 (`8049,14,4,31,0,6,120,0,0,5239`), and `second1d` opens with MUSIC1:

```text
$ head -2 reference/music/second1d/trace.csv
frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe
8100,1,4,39,0,3,130,0,0,5276
$ awk -F, '$1 == 8520 || $1 == 8521' reference/music/second1d/trace.csv
8520,2,44,50,0,3,130,0,1,5588
8521,2,45,50,0,3,130,0,1,0
```

The reload is in **(8049, 8100)**. GLENZ's block-1 wait opens on the same condition as block 2's
and **resets at 8521**; the frame captures first change on 8522 (`reference-indices.md` §7), one
frame after the trace's sample, the sample/publish instant D17 quantises. The restart itself is at **8,056**: a mid-song
restart keeps the previous module's channel volumes, so MUSIC1's order 0 breaks after 4 rows here
(where `second2b`'s fresh player plays 64), and 46 polls back from `second1d`'s first `1:5` (8,102)
is 8,056 (`RESTART_TRACES`; D45, A38).

### 2.3 The block-1 full run: `second1e` and `second1f`

`reference-indices.md` §26.6 carries the capture record.

**The U2E restart's clock starts at 34,055**, in [34,052, 34,055], read as `second3e`'s is (§2.1):
`second1f`'s rows at order 18 sit 33,644–33,647 frames after `second4`'s, so 408.2 + 33,647 ≈
34,055. The boundaries then drift: orders 19…25 start 33,646, 33,644, 33,642, 33,641, 33,639,
33,636 and 33,635 frames after `second4`'s, and the offset stays at 33,635 to the window's end, 11
frames over 3,421 where `second3e` held to one frame.

```bash
awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second1f/trace.csv
awk -F, 'NR>1 && $2!=prev {print $1, $2} {prev=$2}' reference/music/second4/trace.csv
```

`second1e` holds MUSIC1 orders 4…98, with speed 127 from 33692 to its end (the `A7F`, `music.md`
§9.13). In `second1f` the speed word reads **1** from 38319 to 38371 (order 25 row 63 into order
26, about 1.46 frames a row), and the trace publishes row 25 at 38364 and row 27 at 38366: row 26 is
never sampled.

**PANICEND's cue wait (`MAIN/U2.ASM:833-853`) exits at 16630.** From `second1e`'s columns,
`musplus` is −32 from 16363 to 16621, 0 at 16622 (order 40 breaks to order 42 row 0) and **1 at
16630** (order 42 row 1), the first value above 0 after the negative stretch.

**The port against them.** The port's player reproduces `second1e`'s content exactly, its placement
drifting to 8 frames, one past `JITTER_FRAMES`; on `second1f` it publishes row 26 (at 38376) and
drifts by up to 12. `KNOWN_DISAGREEMENTS` pins both (`crates/sr-audio/tests/trace.rs`) and A54
registers them.

The port's own `--block 1`, played headless to 48,277 with `--positions`, against block 1's six
traces (`cargo test --release -p secondreality --test fullrun -- --nocapture`): one offset per
trace, then every frame within the shell's `JITTER_FRAMES` = 3.

```text
second1: offset +0 at 420 (0, 1); 5399 of 5541 frames within 3 (97.437 %), worst 7, 0 past 60; with no offset 5399 (97.437 %); offset +3 at its last row change (5994)
second1c: offset -1 at 5901 (9, 51); 1699 of 1699 frames within 3 (100.000 %), worst 1, 0 past 60; with no offset 1698 (99.941 %); offset -2 at its last row change (7592)
second1b: offset -1 at 7609 (12, 54); 358 of 359 frames within 3 (99.721 %), worst 1, 1 past 60; with no offset 357 (99.443 %); offset -2 at its last row change (7951)
second1d: offset +5 at 8102 (1, 5); 699 of 699 frames within 3 (100.000 %), worst 2, 0 past 60; with no offset 227 (32.475 %); offset +5 at its last row change (8797)
second1e: offset +5 at 8801 (4, 50); 18928 of 25143 frames within 3 (75.281 %), worst 8, 0 past 60; with no offset 23083 (91.807 %); offset -3 at its last row change (33872)
second1f: offset -8 at 34108 (18, 5); 6352 of 14168 frames within 3 (44.833 %), worst 12, 1 past 60; with no offset 13554 (95.666 %); offset +0 at its last row change (47969)
```

The boot's MUSIC0 is the original's to the frame through BEGLOGO; the `MUSIC1@0` session is 5
frames late at its first rows (A38's residual: the port installs the restart at 8,057 and D34's
wait starts its clock at 8,061) and the `MUSIC0@18` session 8 early (A53). The cue wait exits on
the original's row (`42:1`, the port's 16632 against 16630).

## 3. The tick rate, and the rule behind it

A tick is `2.5/tempo` seconds and a row lasts `speed` ticks, so the row rate is
`speed × tempo/2.5` rows a second: a prediction from the player's own `_np_speed` and `_np_tempo`
words, not a fit. MUSIC0's header carries speed 6 and tempo 125 (patched to 120,
`MAIN/STARTMUS.C:257`); MUSIC1's carries speed 3 and tempo 130. `music analyse` measures each run of
constant speed and tempo between its first and last row change:

| trace | run | row intervals | frames | predicted | residual | one row |
|---|---|---|---|---|---|---|
| `hiddenu` | frames 430…3885, speed 8 tempo 120 | 296 | 3455 | 3457.6 | **−2.6** | 11.68 |
| `hiddenu` | frames 3897…6119, speed 6 tempo 120 | 254 | 2222 | 2225.2 | **−3.2** | 8.76 |
| `second1` | frames 420…5994, speed 6 tempo 120 | 637 | 5574 | 5580.6 | **−6.6** | 8.76 |
| `second5` | frames 427…961, speed 6 tempo 120 | 61 | 534 | 534.4 | **−0.4** | 8.76 |
| `second3` | frames 427…2472, speed 6 tempo 130 | 253 | 2045 | 2046.0 | **−1.0** | 8.09 |
| `second3` | frames 2488…2896, speed 3 tempo 130 | 101 | 408 | 408.4 | **−0.4** | 4.04 |

The longest run is reproduced to −6.6 frames, 0.12 %, and every steady run's residual is under one
row. The residual grows with the window because the reference's tick rate is not exactly
`2.5/tempo` (48.06 a second against 48.00 at tempo 120 in `second1`), which is A12.

Two things the traces say that the files do not:

- **The row rate follows the player's speed word, and after a restart that word is not the file's
  byte.** `second3` is MUSIC1 (its `(ord, pat)` pairs match the order list), and over orders 42…45
  after the `SECOND 3` restart (frames 427…2472) `_np_speed` reads **6** where the header byte is 3:
  the row interval is 8.09 frames, speed 6 at tempo 130. From 2488, the crossing into order 47, the
  word is 3 and the interval follows it. What writes the 6 is not measured here.
- **A speed change is followed by a transient.** `second5` at order 25 row 63 sets speed 1: 29 row
  intervals in 42 frames (frames 979…1021), one row a tick. When the speed returns to 6 at 1023 the
  next run measures 35 intervals in 271 frames where the rule predicts 306.6, **−35.6 frames**,
  with five 1–2 frame gaps among the 8s and 9s. `hiddenu` shows the same at its own speed-1 row
  (−37.4 over 40 intervals). The cause is the speed landing mid-row (`music.md` §8).

## 4. The quantisation

The tick clock (48.0 Hz at tempo 120, 52.0 Hz at 130) and the video clock (70.086 Hz) are not
commensurate, so a row boundary lands on whichever frame is current when the player is polled.
`music analyse`, section 3:

| trace | run | gaps |
|---|---|---|
| `hiddenu` | frames 430..3885, speed 8 tempo 120 | 11 frames ×97, 12 ×199 |
| `hiddenu` | frames 3897..6119, speed 6 tempo 120 | 8 ×64, 9 ×190 |
| `second5` | frames 427..961, speed 6 tempo 120 | 8 ×16, 9 ×44, 10 ×1 |
| `second5` | frames 979..1021, speed 1 tempo 120 | 1 ×16, 2 ×13 |
| `second3` | frames 427..2472, speed 6 tempo 130 | 8 ×232, 9 ×21 |
| `second3` | frames 2488..2896, speed 3 tempo 130 | 3 ×1, 4 ×95, 5 ×5 |

Two neighbouring gap sizes carry every steady run. The few outside them are scattered, not
gathered at the edges, and the tool prints each with its position: `second5`'s 10-frame gap is the
18th of 61 intervals (575 → 585) and `second3`'s 3-frame gap the 48th of 101 (2678 → 2681). Why
they fall where they do is not measured; the trace's own row-change frames are the acceptance test,
not any rule stated over them. The transient runs (§3) bunch their outliers at the start:
`second5`'s at positions 1–5 of 35 (and one at 27), `hiddenu`'s at 1–5 of 40.

`second3d` holds MUSIC1's `A7F` (order 98 row 48): the speed word changes 6 → 127 at 17488, and
the run 17489..17668 is 2 row intervals in 179 frames, an 8-frame interval and a 171-frame one, the
127-tick row.

## 5. Which soundcard and mixer rate the reference runs at

`music analyse`, section 4, prints each machine word as recorded at the segment's locate and at
the trace's end. `second5b`'s manifest carries both readings (`machine` and `machine_moved`), as do
the later traces; `second5`, `second3` and `hiddenu` predate the second (§9).

| word | at the locate | when the music plays | what it is |
|---|---|---|---|
| `_zoutputmode` | **5** | 5 | Sound Blaster Pro (`MAIN/STMIK.H:67-74`) |
| `_zmixspeed` | 20000 | **40000** | the mixing rate: 20 kHz before the music engine starts, **40 kHz** once the music plays |
| `_zbufsize` | **4096** | 4096 | the DMA buffer, the largest `MAIN/STMIK.H:88-95` lists |
| `_zpollmix` | 0 | **1** | **the caller polls the player** (`MAIN/STMIK.H:152-176`): the program calls `zpollme`, which `MAIN/COPPER.ASM:466` does once per copper frame |
| `_zspeedadjust` | **0** | 0 | |
| `_zspeedadjust2` | **0** | 0 | `MAIN/U2.ASM:389` writes it only for outputmode 7 or 2, and the card is 5 |
| `_np_mastervol` | 192 | **64** | the object's initialiser is 0xC0 = 192 (static byte at segment offset 0x0911; `crates/sr-assets/tests/omf_symbols.rs`), and `fademusic` (`MAIN/U2.ASM:333-346`) walks it down as a part ends |
| `_np_mastermul` | 0 | **2240** | 0 before the module is loaded, then `0x8C0` (`music.md` §10.3) |

The shell offers 20000 and 40000 among its four rates (`MAIN/U2.ASM:737-743`), and the music runs
at 40000, the port's `MixRate` default. The tick clock does not depend on it: a tick is `2.5/tempo`
seconds of samples at whatever rate, so the rate cancels.

`_zpollmix = 1` means the reference runs the *polled* mode, not the timer-interrupt mode: the tick
clock advances once per copper frame's `zpollme`, which is why `_np_zframe` quantises onto the frame
clock the way §4 measures, and why the port polls once per logic tick.

## 6. `zplus` and `zinfo`

`zplus` takes **0, 1 and 2** in the traces and is a function of the position: over `hiddenu`'s 626
distinct `(ord, row)` pairs, none carries more than one value (`second5`: 131 pairs; `second3`:
358). Its transitions are at order starts, around the `254` entries:

| trace | transitions `(ord, row, from, to)` |
|---|---|
| `second5` | (26, 0, 0→1), (28, 0, 1→2) — MUSIC0's `254` is at order 27 |
| `hiddenu` | the same two |
| `second3` | (45, 0, 0→1), (47, 0, 1→2), (48, 0, 2→1) — MUSIC1's `254` is at order 46 |

### The rule the port computes, and the tests that check it

The port computes `zplus` by the object's own rule, read from the order advance at
`STMIK.300 + 0x442d` (`music.md` §8: bit 0 while the next entry is a `254`, bit 1 when the order
was reached over one, neither cleared, nothing credited behind a seek). `second3`'s `2 → 1` is a
plain advance into order 48, whose successor at 49 is a `254`.
`crates/sr-audio/tests/trace.rs`'s `the_port_reproduces_every_filed_trace` holds the port's
published `zplus` to every filed trace's sequence of values.

**What no trace holds.** The value 3 occurs at MUSIC1's order 26 (between the `254`s at 25 and 27),
which no trace reaches; `crates/sr-audio/tests/player.rs`'s
`a_254_is_stepped_over_and_zplus_records_the_crossing` pins it. Nor does any trace hold an order two past a
marker whose successor is not a marker, so `2` falling back to `0` is the code's behaviour (the
advance clears `zplus` at `+0x442d`), not a measurement. No trace plays a `255` (§10).
`muscode_6` (`DIS/DISINT.ASM:242-282`) is how the parts use the value (`music.md` §11).

**`zinfo` is 0 in every filed trace**: `music analyse`, section 5, prints `zinfo runs: 1 … 0` for
each, over MUSIC0's orders 0…31 and 70…77 and MUSIC1's 0 and 18…98. These traces therefore do not
measure its other values, and D8 records the outcome: route 1 was performed and the word stayed 0.

## 7. What a trace does not hold, and why it moves between sessions

`music analyse` prints each trace's coverage, recomputed from the trace file's frames. In full, for
`second5`:

```
  frames 148..1299 of the requested 147..1299: 1150 sample(s), 3 frame(s) missing (147, 409, 1220) (frames 147..147 of them are the boundary: the segment was located after they had gone) — this trace is NOT per-frame (recomputed from the trace file's own frames)
```

| trace | samples | missing frames |
|---|---|---|
| `second5` | 1150 | 147 (boundary), 409, 1220 |
| `second5b` | 1152 | 144..145 (boundary), 409, 1220 |
| `second3` | 2755 | 142..143 (boundary), 417 |
| `hiddenu` | 6351 | 146..147 (boundary), 410 |
| `second1` | 5797 | 55: 407, 464, 466..467, 476, 502, 511, 513, 521..527, 529, 531, 541, 544..546, 548, 572, 3837, 3860, 3873, 3876, 3879, 3914..3917, 3921, 3926, 3953, 4017, 5567..5569, 5573..5575, 5584, 5590, 5595, 5620, 5626, 5648, 5668, 5674, 5684, 5707, 5710, 5712, 5955 |
| `second1b` | 359 | 7920 |
| `second3b` | 4846 | 10365, 11563, 13574..13575 |
| `hiddenu2` | 1699 | 7335 |
| `second2` | 4597 | 4796, 8192, 8781 |
| `second3a` | 7298 | 5722, 8315 |
| `second3d` | 2799 | 17056 |
| `second1c` | 1699 | 5958 |
| `second4` | 3899 | 4320 |
| `second3e` | 3999 | 21761 |
| `second2b` | 3418 | 405, 1158 |
| `second1d` | 699 | 8568 |
| `second1e` | 25143 | 12207, 15603, 16192, 21926, 24520, 26570, 33261 |
| `second1f` | 14168 | 34116, 37967, 38569, 46264 |

The missing frames are of two kinds, both the tool's: the **boundary**, frames before the segment
was located (only the four traces that start at the boot have one; the others were located at
frames 87…104), and **a frame the guest's counter moved across**, recorded in the manifest's
`skipped` (for `second5`: `409..409 (the counter went 408 -> 410)` and `1220..1220 (the read that
was opened on frame 1220 was still open when the guest moved to 1221)`). Ten of `second1`'s 55 are
frames the re-taken read moved past, which the coverage recomputes and `skipped` does not list: a
long window costs deferred reads (D19).

**Several sit at a video-mode change**, consistent with D17's "a video-mode stop or reset calls
`GFX_EndUpdate` without publishing" but **not measured**: the boot traces' middle frame (409, 409,
417, 410) is always at the boot's first graphics frame (404); `second2b`'s 1158 is GLENZ's
`mov ax,13h` (`GLENZ/MAIN.C:295-299`), where the frame captures cannot grab 1155…1159 either; and
`second5`'s 1220 is inside the ENDLOGO → CRED change (1180, 1440]. In `second1e` and `second1f`,
16192 and 24520 are indices the frame captures also refuse, 33261 sits beside the refused 33260,
15603, 21926, 26570, 34116 and 37967 are where a capture was refused once and taken on a retry,
12207 is TECHNO's start, and **38569 and 46264 are unexplained** (`reference-indices.md` §26.6).

A trace that sampled every other frame would be refused the description "per-frame":
`reference verify` recomputes each trace's coverage from the file and compares it field by field.

## 8. `_np_zframe`, and the word `reference-indices.md` §1.7 measured

`_np_zframe` advances one per **tick**: it is the word the parts set and wait on
(`DIS/DISC.ASM:138-159`, `DIS/DISINT.ASM:412-425`), it starts from 0 with the music (0 at 408, 1 at
410 in `second5`), and it is **reset mid-song**: `second3` goes 1516 → 0 at 2452 with pattern 33
still playing, a part's `_dis_setmframe(0)` (`DIS/DISC.ASM:138-148`; the parts call it at their
start, `LENS/MAIN.C:106`, `GLENZ/MAIN.C:291`, `TECHNO/KOE.C:214`, `PLZPART/PLZ.C:69`). `music
analyse` measures it over the longest run in which it never went backwards:

| trace | longest run | ticks | frames | per frame | per second | the tempo's rule |
|---|---|---|---|---|---|---|
| `second5` | frame 410 (1) → 1299 (609) | 608 | 889 | 0.6839 | **47.93** | 48.00 (tempo 120) |
| `hiddenu` | frame 409 (1) → 6499 (4175) | 4174 | 6090 | 0.6854 | **48.04** | 48.00 (tempo 120) |
| `second3` | frame 411 (2) → 2451 (1516) | 1514 | 2040 | 0.7422 | **52.01** | 52.00 (tempo 130) |

Its rate is a second, independent measure of what §3 measures through the row rate.

`reference-indices.md` §1.7 records a 16-bit word at physical address **`0x105B0E`** advancing
0.7424 times a video frame. Probed beside `_np_zframe` in the same bracketed samples
(`cargo xtask music probe --arg 2 --cycles 30000 --frames 899..3000 --at 0x105B0E`, idle host; the
command is retired, and `git show phase-9:xtask/src/music.rs` holds it), it
is the **unnamed word at segment offset `0x091E`**, two bytes past `_zmixercount` (base +
`0x091C`). It runs 370 → 1927 over frames 900…2999, +51.99 a second, while `_np_zframe` runs 1 → 1401
from its reset at 1112, +52.00 a second: the same tick clock (`2.5/tempo` at tempo 130). What tells
them apart is the reset: `_np_zframe` is zeroed by `_dis_setmframe(0)` and the `0x091E` word is
not, and the difference, 1927 − 1401 = 526, is the ticks from the music's start to GLENZ's reset.
So `_np_zframe` is the parts' copy of the tick count and `0x091E` the count itself. The digits move
by a few ticks between runs and under host load (D17); the rate does not.

## 9. The segment's two views

The locator's needles are found at **two** addresses exactly 1 MB apart, `0x0051F0` and `0x1051F0`,
and they are one memory: the whole 29,607 bytes read through each base are byte-identical at the
locate and at the end of the trace, and the views advance in lockstep. The emulator's memory read
goes through the page tables (`dosbox-automation`, `hardware/memory.cpp:199` →
`cpu/paging.h:293`), so the demo's DOS extender can map one physical page twice.

`music trace` accepts several views only when that equality is proved, and refuses a second copy
that differs (a stale copy would read as a good base until the player moved). The trace reads
through the lowest view, and the manifest records every view, the proof and its frame.

`second5`, `second3` and `hiddenu` predate three manifest fields and are left as filed
(`reference/` is input-only): they carry no `coverage.before_sampling`, their `locate.aliasing`
sentence has a doubled `0x`, and they carry no `machine_moved`. `reference verify` compares
`before_sampling` only when a manifest records one, and `music analyse` recomputes the coverage, so
the boundary is named for them too (D23's additive case).

## 10. The order list: what the player does at `254` and `255`

`music analyse`, section 6, from the order counter's own sequence:

| entry | what the trace shows |
|---|---|
| MUSIC0 `254` at order 27 | **stepped over**: ord 26 → ord 28 without resting on 27 (`second5`, frame 1260; the same in `hiddenu`) |
| MUSIC1 `254` at order 46 | **stepped over**: ord 45 → ord 47 (`second3`, frame 2488) |
| MUSIC0's six `255`s (orders 16, 17, 64, 69, 78, 79) | **not played**: the shell's restarts jump over 16 and 17 (ord 0 → ord 25 at 427) and to order 70 (ord 28 → 70 in `hiddenu`), and `hiddenu` restarts at order 25 again after order 77, pre-empting 78 and 79 |
| MUSIC1's one `255` (order 99) | **not reached**: `second3d` and `second1e` reach order 98 and hold it to the end of their windows |

**`254` is stepped over, and the reference never plays a `255`**: the shell's restarts
(`MAIN/U2.ASM:875-905`: orders 18, 25 and 70) land the player past every one. What the player does
at a `255` is `music.md` §8's reading of the object.

## 11. Tests

| what | where |
|---|---|
| the census and the 278 records; the seven position words as contiguous slots; the frame/tempo/speed offsets; the object's tempo and master-volume initialisers | `crates/sr-assets/tests/omf_symbols.rs` |
| the needles (32 bytes, different records, one occurrence); the locator (a unique match, a second one refused, a needle across a 64 KiB boundary, a short needle, a short last chunk) | `xtask/src/music/locate.rs`'s own tests |
| the window (`A..B` half-open), the row round trip, the coverage's gaps and boundary run; back-to-front and overflowing windows are findings, never crashes | `xtask/src/music/format.rs` and `trace.rs`, `xtask/tests/music.rs` |
| machine words recorded twice when they move, once when they do not, and a word above 32767 printed both ways (`a_machine_word_above_32767_is_printed_both_ways`) | `xtask/src/music/trace.rs`, `analyse.rs` |
| the `zplus` rule against MUSIC1's order list, the `3` included | `crates/sr-audio/tests/player.rs` |
| `music symbols` prints the map; the trace refusals (where it may be written, the window, the keys, `--force`) | `xtask/tests/music.rs` |
| the traces as references: rows, columns, coverage and skipped frames; `reference verify --tree reference` passes; **a flipped byte of a trace fails it** (exit 1, both digests named); an unknown `kind` and an unsupported coverage claim are findings | `xtask/tests/music.rs` |
| `music analyse`'s numbers and the boundary named | `xtask/tests/music.rs` |
| **the anchor is per recipe** (408, 399.6, 401.1), and a window's extrapolation is not one | `xtask/tests/music.rs`'s `the_anchor_is_the_same_in_every_filed_session` |
| the port's player against every trace, `zplus` included | `crates/sr-audio/tests/trace.rs` |
