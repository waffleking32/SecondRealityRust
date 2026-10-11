# Reference indices — one row per part, and how the indices are found

Phase 1's reference ledger. One row per part, and for each part the boot indices its keyframes
were captured at, the geometry of those frames, what is on screen at each, the `sha256` of every
file the capture wrote, and the command that produced it — plus the re-capture that shows the
reference reproduces.

The rows are the deliverable; the method below them is not commentary. §1 is the procedure every
later capture task (B3, B4, B5) repeats, written down because each step of it cost a measurement
to find, and because two of the rules the plan gave for it turned out to be wrong in ways that
only show up on a part whose keyframes are far apart — which every part after BEGLOGO is. §2 is
the table. §3 is BEGLOGO, the first row.

**The recipe every capture in Part B uses** is fixed and is in the manifests:

```
--arg <block> --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000
```

`--boot autoexec` is B1's recipe (D17): the guest's own `[autoexec]` runs the mount, `AUTOTYPE`
answers the setup screen on the emulated clock, and the emulator runs silent, so the same boot
index is the same screen in every run. A capture's manifest records `boot`, `cycles` and `keys`,
so a frame set that was captured some other way says so. Indices are **boot-relative**: frame 0
is the emulator's first rendered frame at process start (D14), the setup screen's text is up at
16, and the demo's first graphics frame is 404.

---

## 1. The method

### 1.1 Sweep, then look at the sheet

```
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames <a dozen ascending indices> --out out/<part>-sweep --sheet out/<part>-sweep.png
```

A dozen indices spanning the few seconds the part is expected to occupy, written into `out/` —
**never into `reference/`**. The sweep is discovery: what it captures is thrown away once the
indices have been read off the sheet, and the sheet is how they are read. The capture prints each
tile's position as it lays it out (`r0c0 frame 430 (frame_000430.rgb)`), so the sheet and the log
say which index is which tile and nothing has to be inferred from a grid position.

Read three things off it: where the part's picture starts, where it holds still, and where it
stops being this part. Where a part has a held screen, that is where the keyframes belong (§1.4).

The sheet holds what the *successful* capture took. A sweep that crosses a part boundary never
succeeds (§1.2), so the first frames of the *next* part come from the refusal's index and from the
first sweep taken inside that part — not from this sheet.

### 1.2 Where a part ends: the refusal is the discovery

A capture is one geometry (D15). A sweep that runs from one part into the next changes geometry
under it — BEGLOGO is 320×400 Mode X, GLENZ is 320×200 chain-4, and `video/frame` follows the
guest rather than normalising — so the capture stops at the first index of the new geometry:

```
error: the display changed geometry between frames: frame 1170 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

That refusal is the boundary, and it is not to be worked around: sweep ascending, read the index
it names, and **the change lies between the last index that captured and the one that was
refused**. Two things about the mechanics matter:

- **A refused capture writes no manifest.** The frames taken before the refusal are on disk in the
  output directory (`out/beglogo-boundary/` holds `frame_001110.rgb`, `frame_001130.rgb` and
  `frame_001150.rgb`), but the manifest is written only after every requested index has been read,
  so nothing in that directory is a capture. The indices come from the refusal's message and from
  the narrower runs, not from the leftover frames.
- **Indices just below the change are grab-hostile.** The changeover stalls the renderer, and a
  requested index in that window is refused for a different reason — measured on BEGLOGO: 1155
  (`the emulator reached frame 1156 before frame 1155 could be captured`), 1158 (twice, reaching
  1159), and 444 in a dense probe of the fade's first frames. So narrow the boundary with **sparse
  single-index probes from both sides** rather than a dense run: a dense list spends its takes in
  the window that refuses them, and one miss refuses the whole capture.

BEGLOGO's boundary, measured that way, is in §3.4.

### 1.3 Two rules for a list of indices

**A run is read after it is taken.** Requested indices closer together than `INDEXED_READ_FRAMES`
(25) are all grabbed before any of them is read, because one indexed read (the DAC palette is 1024
of its requests) costs ~20 rendered frames and used to sit between two grabs (D19). The price is
deferred reads: a run of *k* indices spends ~20·*k* rendered frames reading afterwards, and the
next requested index must be further away than that or it arrives during the reads and is refused.
Measured again here, at 30000 cycles on this emulator: `--frames 429,430,431,479,480,481,…`
captures the three and then refuses the fourth — `the emulator reached frame 488 before frame 479
could be captured`, so three reads cost 57 frames, ~19 each. That is the number the plan's "~25·*k*"
names the ceiling of; leave **~20·*k*** after a run of *k*.

**A read's palette is the palette at read time, not at grab time.** The frame's pixels — RGB and
indices alike — are the ones taken, but the 256 DAC entries are read live when the run is read, so
a palette that moves in between makes the pair incoherent and D15's rule keeps the frame RGB-only.
Two consequences worth knowing before reading a sweep's output: **a screen whose palette is fading
is always RGB-only**, however still its indices are (the read is 20+ frames late, and that is
20+ fade steps); and **a frame inside a batch is read at the end of that batch**, so a batch that
spans a palette change is RGB-only even where a single-index capture of the same index keeps its
indices. Both are measured: BEGLOGO's 470 and 560 were RGB-only in all three sessions that took
them, and `out/beglogo-fade` — one batch across the whole fade — has 430–570 RGB-only while the
single-index capture of 430, taken at its own palette, has its indices.

"rgb only" in a capture's output therefore means *the indices would not have been honest*, which
is either a moving screen or a late read; it never means the RGB frame is the wrong frame.

**The reference holds keyframes only; the capture compared against it holds support frames.** The
asymmetry is D19's and the reason is `compare`'s rule: it pairs *every* frame of the reference, so
a support frame sitting in the reference is itself a reference index whose counterpart lies one
further out. The plan's form for the partner is `--frames <i-1,i,j,k,k+1>` — one support beyond
each *end* of the keyframe range. **That is only enough when the keyframes are contiguous**, which
is the case B1b measured it on. Two corrections, both from this task:

1. **Each keyframe that moves needs a support on each side, not just the outermost.** A session's
   phase is uniform across a whole capture (D17 measured it across five indices; nothing since has
   shown a mixed one), so a partner one step out of phase has to answer *every* keyframe from its
   neighbour — an interior keyframe answered from its own index pairs at offset 0 and reports the
   one-step difference (exit 1) rather than aligning. The partner therefore holds `K-1, K, K+1`
   around each keyframe that changes from frame to frame.
2. **A keyframe whose neighbours are the same picture needs no support at all.** A held screen's
   ±1 candidates are the same bytes, so the search finds offset 0 whatever the partner's phase is.
   That is measured, not assumed, per keyframe: 429, 430 and 431 are byte-identical (and equal to
   410), and so is every sampled index of the hold from 580 to 1100 — the one shared index that
   did differ, 1110, is the collapse's edge and is not a keyframe (§3.3) — so BEGLOGO's white and
   held keyframes carry no supports and only the two fade keyframes do.

Both corrections cost something the plan's form does not: because a keyframe's three supports are
one batch, **consecutive keyframes have to be ~60 frames apart in the partner's list** — that is
the measured cost of a 3-index batch (57 frames, above), and the keyframes are laid out to it with
whatever margin the part allows. Two keyframes 50 apart cannot both be supported; BEGLOGO's are 90
and 340 apart.

### 1.4 What "held" means, and why keyframes are chosen there

A keyframe is *held* when the frames either side of it are the same picture, because that is where
the reference's reproducibility is strongest and where a port's diff means something. A palette
fade over a held image is held in the index plane and moving in the palette: BEGLOGO's fade is
exactly that, and its frames are RGB-only because the palette changes while the indexed read
runs (D15's coherence rule) — 22 of the 28 frames of `out/beglogo-fade` came back RGB only, and
both fade keyframes did. A logo hold is held outright. Held is checked, not assumed: the sweep's
frames are hashed against each other (§3.3), and a shared index captured by two runs is the same
bytes or it is not.

### 1.5 Cycles

Everything in Part B is captured at `--cycles 30000`, the recipe B1 measured (D17). Phase 0's
`reference/check_a` was captured at 25000 and re-captured at 30000 in B1, so the tree has one
convention — but whether that matters was measured rather than assumed, because if a boot index
only means something next to the cycles that produced it, every capture would have to be compared
only with a capture of the same vintage:

**capturing the held index 900 at 25000 and at 30000, everything else identical, gives byte-
identical files** — `frame_000900.rgb` is `cbdc67eb…` and `frame_000900.idx` is `e10481c7…` in
both, and `cargo xtask compare` reports `verdict: identical`, 0.0000%, offset 0, exit 0. The
cycles change how long a capture takes in wall time and nothing about what it contains: the
counter counts emulator-rendered frames, and one frame is one emulated vertical refresh whatever
the cycles. `reference/check_a` (30000) can therefore be compared against any Part B capture.

### 1.6 Two things ALKU measured that the rules above do not cover

**A frame can come back RGB-only because of how the oracle reads a screen, not because the screen
moved.** The coherence check is a comparison of the grabbed RGB against the RGB re-derived from the
planes and the palette read a moment later (D15), and it fails whenever the two disagree — which the
rules above read as "the screen moved". There is a second way to disagree, and ALKU takes it: the
read assembles the planes at **`width/4` bytes a row** (`xtask/src/oracle/dosbox.rs:620-629`), which
is the packed Mode X layout `reference/parts/beglogo`'s screens use, but ALKU's screen is 176 bytes
a row — `ALKU/TWEAK.ASM:40-46` sets CRTC 0x17 = `0xe3` (word units) and 0x13 = `0x58` (88 units,
176 bytes), and `ALKU/MAIN.C:26` strides its own rows by 176 (`char (* far vvmem)[176]`, used at
`MAIN.C:165-166`, `:367-368`). With the wrong stride the assembled indices are not the screen's, so
**every ALKU frame with a picture on it comes back RGB-only** — in each of the ten captures that
took one, whatever the batch shape — while the frames whose pixels are all a black DAC entry pass.
That the cause is the read and not a moved screen is measured: the same index is RGB-only in every
session, and its RGB bytes are identical to the reference's (1719 and 2719 below). Consequences worth
carrying: `compare` runs this part's rows through the **RGB path**, where the 6→8-bit widening
`(c*259+33)>>6` is load-bearing (`docs/decisions.md:422-423`); and PAM, whose `PAM/TWEAK.ASM:18-41`
is the same `tw_opengraph` with the same 176-byte rows, is expected to behave the same way (§5).

**A moving screen's session-to-session difference can be an exact integer pan, which no ±1 frame
support can bridge.** D17's moving case is a screen that is redrawn continuously: a session phase
difference of one frame leaves a **0.2–0.6 % residue**, because the one-frame shift is not uniform
across the picture. ALKU's scrolled picture is a different kind of moving screen — it advances its
window by one pixel per step and holds the same bytes in between (`ALKU/MAIN.C:7` `#define SCRLF 9`,
`MAIN.C:87` `for(f=60;a<320;...)` around `do_scroll(1)`), so the whole picture is identical between
sessions **after a whole number of pixels**: measured, the reference's 5859 against one session's is
**0 pixels differing of 95,700** after a 1-pixel shift and against another's after a 2-pixel shift,
while at offset 0 they differ by 10.975 % and 15.2797 % (§4.4). Two things follow, and B4/B5 should
expect them wherever a part pans:
- **The ±1 support rule buys nothing here.** The support window is ±1 *frame*, and the difference is
  a whole number of 9-frame scroll steps, so no choice of neighbouring index recovers the
  reference's picture.
- **A row is still worth having.** The reference frame is one exact picture; a port can be required
  to produce *that* picture, and the row says what the sessions can differ by instead of hiding it.

Held screens are untouched by either: 900, 1719 and 2719 below are byte-identical in every session
that took them.

---

## 2. The index table

One row per part. `keyframes` are the boot indices in `reference/parts/<part>/`; `the partner`
is the scratch capture the re-capture comparison was run against, which also holds the support
frames and is not a reference. A part whose screens span two geometries gets one row per screen
(B3 step 2). Every row cites its own command and its per-frame hashes in the section below the
table.

| part | boot | geometry | keyframes | re-capture | re-capture result |
|---|---|---|---|---|---|
| BEGLOGO | `SECOND 2` | 320×400 Mode X | 430, 470, 560, 900 | `out/beglogo-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}` |
| ALKU | `SECOND 1` | 320×400 Mode X, 176-byte row stride | 900, 1719, 2719, 5859 | `out/alku-recapture2` | exit 1 — 900/1719/2719 identical, 0.0000%, offset 0; 5859 differs by a 2-pixel pan, 15.2797% at offset 0 and 0.0000% once shifted (§4.4) |
| U2A | `SECOND 1` | 320×200 | 6500, 7300, 7640, 7760 | `out/u2a-recapture` | exit 0, `verdict: identical`, 0.0000%, offsets `{0: 4}` — including the two moving keyframes, which landed in the same alignment group (§5.4) |

### Adding a row

1. Sweep and find the indices (§1.1, §1.2); pick the keyframes at held or structurally
   significant moments (§1.4).
2. Capture the reference: keyframes only, into `reference/parts/<part>/`, with the sheet going to
   `out/`.
3. Capture the partner into `out/<part>-recapture`, holding `K-1, K, K+1` around every keyframe
   that changes (§1.3).
4. `cargo xtask compare --reference reference/parts/<part> --rendered out/<part>-recapture
   --out out/<part>-recapture-diff`; record the exit code, the offsets and the percentage.
5. Fill the row in, and add the part's section below with the commands, per-frame hashes and what
   is on screen at each index.

A row that is a *moving* screen and exits 1 is re-run once (D19) — same alignment group and it is
byte-identical; different group again and the row records the offset and the residue, which is a
statement about the oracle rather than a failure of the reference. **No pixel tolerance is added
anywhere**: the alignment is searched, the pixels stay exact.

---

## 3. BEGLOGO

`SECOND 2`'s first part: the Future Crew logo, white, fading in over a held picture. 320×400
Mode X. What *this part* draws spans the demo's first graphics frame, 404, to the end of its fade
in 571–579 (§3.3) — about 2.5 seconds. The screen then stands still until GLENZ closes it down at
~1115 and changes mode in (1154, 1160] (§3.4, §3.5), so the 320×400 screen lasts roughly 750
frames — about 10.7 seconds — of which the first ~176 are this part's drawing. That distinction is
§3.5's, and it matters to Phase 2.

### 3.1 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 430 | the opening **white screen** — the whole frame is palette index 15, whose only palette entry is `(63,63,63)`, so every pixel is `255,255,255`. The screen is white from the part's first frame (404) until the fade's first step; 429 and 431 are byte-identical to it (§3.3). | `.rgb` + `.idx` (1 palette entry) | rgb `85789170…` |
| 470 | the **early fade** — the logo is visible and pale. 27 distinct colours; its mean absolute difference from the held picture is 18.448 of the white screen's 23.815, so it is ~23 % of the way through the white→picture palette ramp. | `.rgb` only | rgb `d0abd8cc…` |
| 560 | the **late fade** — the same picture ~93 % faded in (mean difference 1.718; 52 colours). The last sampled frame that still differs from the finished picture is 570 (§3.3). | `.rgb` only | rgb `05a130da…` |
| 900 | the **held logo** — the finished picture, 57 palette entries, held from 580 to 1110. | `.rgb` + `.idx` (57 palette entries) | rgb `cbdc67eb…`, idx `e10481c7…` |

430 and 900 are carried with indices because their palettes hold still across the indexed read;
470 and 560 are RGB-only because the fade moves the palette while the read runs, which is D15's
coherence rule refusing to record indices that do not widen back to the frame taken.

**What that means for the comparison is worth stating, because it is not the usual case.** With two
of the four frames RGB-only, this row's `compare` run reports `mode: mixed` and **two of the four
frames that judge Phase 2's exit criterion go through the RGB path** — where the DAC's 6→8-bit
widening, `(c*259+33)>>6`, is load-bearing rather than bypassed (`docs/decisions.md:422-423`; an
indexed comparison never depends on it, which is `compare.rs`'s reason for preferring indices). So
a port whose palette widening differed would show up on 470 and 560 and not on 430 and 900, and
Phase 2 should expect exactly that pair of readings rather than treating the row as four
indexed comparisons.

Full hashes and the commands:

| index | file | `sha256` |
|---|---|---|
| 430 | `frame_000430.rgb` | `857891706b1698637cb83d2e7a6e005c5af26909187cc1426195a088fd1e2561` |
| 430 | `frame_000430.idx` | `283babe8ef08242dfe2917f4eb6e7e8f050cc70891ccbda5c79ba851c01e130d` |
| 470 | `frame_000470.rgb` | `d0abd8cc06b5cfb4923d297effb54dfb4fcddd0a36950090f3a26875002f8e79` |
| 560 | `frame_000560.rgb` | `05a130da7b95c3527ed5699a72ebfb389f4b74881d9c7a079b8159908af8e76a` |
| 900 | `frame_000900.rgb` | `cbdc67eba977d5a58f09e5e14cfd04f04acdee716304ec3f15b830142cae1abc` |
| 900 | `frame_000900.idx` | `e10481c71b2ac341c8424a071e59c0a5c652c0517687f0c1f7d6817d5c004b08` |

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 430,470,560,900 --out reference/parts/beglogo --sheet out/beglogo.png

# the partner: the same keyframes plus a support frame each side of each fade keyframe
cargo xtask reference capture --arg 2 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 430,469,470,471,559,560,561,900 --out out/beglogo-recapture
```

The sheet's four tiles, in the order the capture laid them out: **r0c0** frame 430 is white;
**r0c1** frame 470 shows the logo pale and washed out; **r1c0** frame 560 shows it nearly solid;
**r1c1** frame 900 is the finished picture. Thumbnail-sized, 560 and 900 are hard to tell apart —
which is what "93 % faded" means — and the two are not the same bytes (`05a130da…` against
`cbdc67eb…`), so the middle of the ramp is where the fade is visibly still moving.

### 3.2 The re-capture: exit 0, identical, offsets `{0: 4}`

```bash
cargo xtask compare --reference reference/parts/beglogo --rendered out/beglogo-recapture \
    --out out/beglogo-recapture-diff
```

Two boots, two sessions:

```
frame 430: 0.0000% differing (0/128000 px), max delta 0
frame 470: 0.0000% differing (0/128000 px), max delta 0
frame 560: 0.0000% differing (0/128000 px), max delta 0
frame 900: 0.0000% differing (0/128000 px), max delta 0
verdict: identical
exit: 0
```

`mode: mixed` (430 and 900 compared as indices, 470 and 560 as RGB), every offset 0, and all four
frame files `sha256`-identical between the two directories. The supports were not needed for this
pair — the two sessions landed in the same alignment group — but they are what makes the result
mean something if a later pair does not.

**This is the strong claim and it is worth saying what kind of screen it is a claim about.** 430
and 900 are held screens, and 470 and 560 are a palette fade over a held picture: the indices do
not move at all, only the DAC does. All four are byte-identical across sessions, and the *held
logo at 900 is byte-identical in five separate sessions* (the sweep, this reference, the partner,
and the 25000- and 30000-cycle probes of §1.5), which is the strongest form of the claim this
phase can make. What is *not* claimed here is that a screen the demo is continuously drawing
repeats this way — D17 measured that it does not, about half the time, and no part captured so
far has tested it.

### 3.3 The fade curve, measured

`out/beglogo-fade` captured 430–700 in tens. The white screen is `255,255,255` at 430 and 440
(one distinct colour); 450 is the first frame with more than one colour (5); of the samples, 570 is
the last that still differs from the finished picture (mean difference 0.156, max channel delta 5)
and 580 onwards is the held picture exactly — so the ramp's first changing frame is in 441–449 and
its last in 571–579, and nothing in the sampling narrows those two windows further.

| index | 440 | 450 | 470 | 500 | 520 | 550 | 570 | 580 |
|---|---|---|---|---|---|---|---|---|
| distinct colours | 1 | 5 | 27 | 43 | 48 | 51 | 52 | 52 |
| mean difference from the held picture | 23.815 | 22.0 | 18.448 | 12.927 | 9.236 | 3.767 | 0.156 | 0 |

That is a white→picture ramp of ~129 steps, one per frame — which is what `BEG/BEG.C:80-85` is:
`for(c=0;c<=128;c++)` around `dis_waitb()` and a `setpalarea`, one step per vertical blank. The
picture is blitted into video memory once, before the fade starts (`BEG/BEG.C:73-78`), and the
white opening is the `memset(vram,15,…)` and the all-white palette at `BEG/BEG.C:20-33` — so the
part's picture never moves; only the palette does.

Cross-run identity of the shared indices, which is what makes the two fade keyframes worth
choosing:

| index | sessions | result |
|---|---|---|
| 410, 429, 430, 431 (white) | 4 captures | byte-identical, one `sha256` |
| 470 | 3 captures | byte-identical |
| 560 | 3 captures | byte-identical |
| 690, 1040 (held) | 2 captures each | byte-identical |
| 1110 | 3 captures | **2 identical, 1 different** — its last scanline only (320 px of 128,000, max channel delta 255) |

1110 is the edge of the transition in §3.4: one session's collapse had already begun there. It is
the only shared index in the whole part that ever differed, and it is ±1 frame of the collapse's
start, not a difference in the picture.

### 3.4 Where the part ends, and what is in between

Sweeping up, `out/beglogo-boundary` captured 1110, 1130 and 1150 at 320×400 and was refused at
1170:

```
error: the display changed geometry between frames: frame 1170 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

Narrowed from both sides with sparse probes: **1154 is still 320×400** (captured alone, `out/
b2-boundary-probe4`), **1160 and 1164 are already 320×200** (`out/b2-boundary-probe2`, a capture
that starts at the new geometry and so is not refused). Indices 1155 and 1158 could not be
captured at all, in three attempts between them — `the emulator reached frame 1156 before frame
1155 could be captured` and, twice, `…frame 1159 before frame 1158 could be captured` — so the
change lies in **(1154, 1160]**, with the window's middle un-grabbable rather than un-sampled.

Two things sit in that window and neither is a keyframe:

- **From ~1115 the picture collapses.** `out/beglogo-tail` (1000–1150 in tens) shows 1000–1110
  byte-identical to the held frame; 1120, 1130, 1140 and 1150 all differ from each other and from
  1110 (128,000, 122,880, 106,240 and 77,760 pixels between consecutive samples). The picture
  shrinks vertically toward a band around the middle of the screen — at 1130 it occupies rows
  46–377 with black above and below, at 1140 rows 106–348, at 1150 a flat grey band at rows
  191–307 — and the surround goes to black. It is a transition, not a held screen and not a
  freeze. A sheet shows it only from a sweep that stops short of the boundary; a sweep that
  crosses it is the one that gets refused.
- **The GLENZ side of the boundary is B3's row**, and its sweep starts inside GLENZ (1160, say)
  for the reason in §1.2.

### 3.5 What the collapse is: GLENZ's opening, not BEGLOGO's ending

This was left open in this task's first pass, with two `BEGLOGO.EXE` builds as the candidates, and
that was a dead end because the collapse is not BEGLOGO's at all. Fix round 1's review attributed
it, and the attribution is checkable in the sources and against these frames: **it is the first
thing the next part draws**, GLENZ's `zoomer2()` — a vertical close run before GLENZ's own mode
set, in the outgoing 320×400 geometry. `GLENZ/MAIN.C:288-303` is `dis_partstart()`, a wait on the
music (`while(!dis_exit() && dis_musplus()<-19);`), then `zoomer2()`, and only after it `mov ax,13h`
/ `int 10h`. Three things in the frames match the source line for line:

- **The band law is `zoomer2`'s.** It advances a quadratic — `zya++` then `zy += zya/4`
  (`GLENZ/ZOOMER.C:41-44`), so `zy(n) = n(n+1)/8` — and fills a top band through `zy` and a bottom
  band tracking `zy2 = 125*zy/260` (`ZOOMER.C:45-58`). Both edges follow it: the top of the
  non-black region is row 11 at 1120, 46 at 1130, 106 at 1140 and 191 at 1150, whose second
  difference is **exactly 1/4 row per frame²** — which is `n(n+1)/8`'s second difference, and the
  law the counter in `zy += zya/4` gives — and the bottom black edge at 1130, 1140 and 1150 is
  `399 - 125*zy/260` for the `zy` the top edge implies, within a row each time (378, 349, 308). The
  review that attributed this probed the band two frames apart and measured the top edge advancing
  14, 15, 16, …, 21 rows per two frames, which is the `(n+1)/2` that `zy += zya/4` gives.
- **The grey it converges to is `zoomer2`'s target.** The picture entries fade to a flat grey —
  `c` is `frame` capped at 32, `b = 32-c`, `pal2[a] = (pal1[a]*b + 30*c) >> 5`, applied with
  `setpalarea(pal2,0,128)` (`ZOOMER.C:59-68`) — so the target is six-bit **30**, which widens to
  **121**. The band measured at 1150 is exactly `(121,121,121)`.
- **Why the bands stay black while the picture greys.** `zoomer2` writes palette index **255**
  into both bands (`memset(v,255,80)`, `ZOOMER.C:48` and `:56`) and fades only entries 0–127, and
  `BEG/BEG.C:25-34` is what leaves entry 255 at `(0,0,0)` — white into 0–254, black into 255. The
  polarity is that combination and not a coincidence: a fade that touched 255 would take the bands
  grey with the picture.

**And a knock-on Phase 2 should be told rather than left to discover:** BEGLOGO's own drawing ends
with its fade. `BEG/BEG.C:86` is the last `setpalarea` and `BEG.C:87` closes `main()` — there is no
loop after it — so the 580–1110 hold is not BEGLOGO holding anything: it is the loader and GLENZ's
music wait leaving BEGLOGO's last screen up for ~530 frames, and the collapse then plays on top of
it. The leading reading is that **"reproduce index 900" means reproducing a hold that belongs to
the next part's music wait or to the loader, not to a BEGLOGO drawing loop**, and it is a reading
rather than a settled fact because nothing here measured the port. It is also robust to which
BEGLOGO build shipped: the other source in the tree would itself wait on a music code before
exiting (`ENDPIC/BEG.C:45-48`, capped at 300 frames), and 300 frames is less than the ~530-frame
hold, so GLENZ's wait is in the hold either way. What it does not change: 900 is still the right
picture and the right bytes, and no keyframe sits in the collapse.

---

## 4. ALKU

`SECOND 1`'s first part: three title cards over black, then a scrolled horizon picture, one
geometry (320×400) throughout. It is the longest part captured so far — from the demo's first
graphics frame to a mode change in (5950, 5960], about 79 seconds — and it is the phase's first
part with text, with a palette-driven card, and with a *moving* picture. The part is
`ALKU/MAIN.C`: each card is gated on the music (`while(dis_sync()<1..4)`, `MAIN.C:59,66,72,79`),
drawn once (`prtc`, `MAIN.C:61-63`) and then faded in, held and faded out
(`dofade(fade1,fade2); wait(300); dofade(fade2,fade1)`, `MAIN.C:64` — 64 steps in, 300 frames held,
64 out, one step a frame per `dofade`'s own loop, `MAIN.C:301-311`); after the fourth sync the picture
fades in over 128 frames (`cop_dofade=128`, `MAIN.C:83`) and `for(f=60;a<320;...)` scrolls it a step
at a time while five text batches are laid over it (`MAIN.C:87-135`).

### 4.1 The sweeps, and what the sheets showed

```
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 420,620,820,1020,1220,1420,1620,1820,2020,2220 --out out/b3-alku-sweep1 \
    --sheet out/b3-alku-sweep1.png
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 2420,2720,3020,3320,3620,3920,4220,4520,4820,5120 --out out/b3-alku-sweep2 \
    --sheet out/b3-alku-sweep2.png
```

Both at 320×400, both exit 0. `b3-alku-sweep1.png`'s tiles, in the order the capture printed them:
**r0c0–r1c1** (420, 620, 820, 1020, 1220, 1420) are plain black; **r1c2, r1c3** (1620, 1820) are the
card `A / Future Crew / Production`; **r2c0** (2020) is black again; **r2c1** (2220) is
`First Presented / at Assembly 93`. `b3-alku-sweep2.png`: **r0c0** (2420) the same
`First Presented` card, **r0c1** (2720) `in` over the **Dolby Surround logo**, **r0c2** (3020)
black, **r0c3** (3320) the landscape picture, faded in but already scrolling; **r1c0–r2c1**
(3620, 3920, 4220, 4520, 4820, 5120) the same picture alternately bare and with a text batch —
`Graphics / Marvel / Pixel` at 3920, `Music / Purple Motion / Skaven` at 4520,
`Code / Psi / Trug / Wildfire` at 5120.

So the sheet alone says: black → card → black → card → black → card → picture with texts. The
minimum over each card's position is what the sweep cannot give, and §4.2 measures it for the first
one. A third sweep (`out/b3-alku-sweep3`, 5420…8120 step 300) was **refused**, which is how the
part's end was found (§4.5); the two frames it took before the refusal (5420, 5720) show the picture
bare at 5420 and with the last text batch `Additional Design / Abyss / Gore` at 5720.

### 4.2 The card, measured frame by frame

`out/b3-alku-card1` took every 20th frame from 1500 to 2000 in one run (26 indices, exit 0, 35 s) —
one batch, so every frame in it is the frame that was asked for:

| index | 1500–1540 | 1560 | 1580 | 1600 | 1620–1900 | 1920 | 1940 | 1960 | 1980–2000 |
|---|---|---|---|---|---|---|---|---|---|
| mean sample value | 0 | 1.18 | 2.56 | 3.97 | 4.30 | 3.39 | 2.01 | 0.61 | 0 |
| `sha256` | `fec9afb5…` ×3 | `ea09bd61…` | `5ca8d435…` | `e56da741…` | `6a27ec50…` ×15 | `55bc5ebe…` | `0bb265c7…` | `3d577743…` | `fec9afb5…` ×2 |

That is the source's shape exactly: black until the first fade step lands in (1540, 1560], 64 steps
of fade-in, a hold the sampling shows as one `sha256` at all fifteen samples from 1620 to 1900
(280 frames sampled, no change), 64 steps out, black by 1980. The card's `6a27ec50…` is the same
bytes in five other captures (1620 and 1820 of the sweep, 1719–1721 of `out/b3-alku-probe`, the
reference and both partners), so the hold is a held screen in the sense §1.4 means it, measured
across six sessions rather than assumed from the source.

The same three-sample pattern holds for cards 2 and 3 where they were sampled (`out/b3-alku-probe`
took 2419/2420/2421 and 2719/2720/2721 and got one `sha256` per triple, `6a2de862…` and `5e3c3884…`),
and the black screen is one `sha256` (`fec9afb5…`) at 420, 620, 820, 1020, 1220, 1420 and 2020
(sweep 1), 1500, 1520, 1540, 1980 and 2000 (the card run), 3020 (sweep 2), 412 and 416
(`out/b3-alku-start`) and 899–901 (the probe) — **eighteen indices in five captures**, plus 900 in
the reference and both partners. The frame is black without being one index: the reference's 900
uses sixteen distinct indices (0–11, 19, 20, 21 and 47) whose palette entries are all `(0,0,0)`,
which is also why the coherence check keeps its indices (§1.6).

### 4.3 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 900 | the **black screen** — every pixel is `(0,0,0)`, from sixteen distinct indices whose DAC entries are all black. The part opens on it (the first graphics frame is 404, D17; the earliest index this boot can grab is 412, and 404–410 are refused as misses) and holds it until the first card's fade starts in (1540, 1560]. About 16 seconds of the part. | `.rgb` + `.idx` (16 palette entries) | rgb `fec9afb5…`, idx `1b2bb3e3…` |
| 1719 | **card 1** — `A`, `Future Crew`, `Production` in the FONA font, mid-hold (fade in ended ~1615, fade out starts ~1905). | `.rgb` only | rgb `6a27ec50…` |
| 2719 | **card 3** — `in` over the Dolby Surround logo glyphs, mid-hold (`prtc`, `MAIN.C:74-76`). | `.rgb` only | rgb `5e3c3884…` |
| 5859 | the **scrolled picture**, bare of text, mid-step: it is the same bytes at 5859, 5860 and 5861 (measured, `out/b3-alku-probe`), so its ±1 neighbours are the same picture *in this session*. | `.rgb` only | rgb `be6f6c32…` |

The three held keyframes are the part's three distinct held screens (black, a text card, the logo
card); card 2 at 2419 is deliberately *not* a keyframe — it is the same code path and the same kind
of screen as 1719 with different strings (`MAIN.C:68-69` against `:61-63`), so a fourth held card
would buy nothing a fourth slot should buy. The fourth slot goes to the picture, which is the only
screen this part has that moves.

**Why 5859 is mid-step and not at a step boundary**: 5859, 5860 and 5861 are one `sha256`, so the
step boundary is at 5862 or later. It is worth saying what that does and does not buy: it makes the
*supports* cheap, but it is not what makes the frame reproducible — across sessions this frame is
not (§4.4).

Each of 900, 1719 and 2719 carries **no support frames**, and that is a measurement rather than a
saving: 899/900/901, 1719/1720/1721 and 2719/2720/2721 are each byte-identical within their triple
(`out/b3-alku-probe`, one `sha256` each), so a support frame there is the same picture and could
only ever answer at offset 0 (§1.3). 5859's support window was captured anyway, because its ±1
identity is a property of where the step boundary happens to fall, not of the screen.

Full hashes of the four files in `reference/parts/alku/`:

| index | file | `sha256` |
|---|---|---|
| 900 | `frame_000900.rgb` | `fec9afb531a8e036eba1d81651896e1b2c1f78b0234dbadc8e7549563f09407b` |
| 900 | `frame_000900.idx` | `1b2bb3e331529400a02a4e60b5b74dfd2857634285364fbf520c89bfc622ad0f` |
| 1719 | `frame_001719.rgb` | `6a27ec502abe606d39c25af842594335fb71d30e14a1f245e9492f3b4148cebe` |
| 2719 | `frame_002719.rgb` | `5e3c38846788c1cc4d8d23c65dfdb620779d9691b598e97ee2d79a3f4f0f0bd3` |
| 5859 | `frame_005859.rgb` | `be6f6c32e8a328a04a93c5e54129830f58c187c5deb4dbb399e1819f261240d2` |

### 4.4 The re-capture: three held frames identical, the picture a whole pan away

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5859 --out reference/parts/alku --sheet out/alku.png

# the partner: the same keyframes plus a support frame each side of the picture keyframe
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5858,5859,5860 --out out/alku-recapture

# the re-run (§2's rule for a moving row that exits 1), with one extra frame a scroll step earlier
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 900,1719,2719,5850,5858,5859,5860 --out out/alku-recapture2
```

First comparison (`out/alku-recapture`, exit 0 as a capture):

```
frame 900:  0.0000% differing (0/128000 px), max delta 0
frame 1719: 0.0000% differing (0/128000 px), max delta 0
frame 2719: 0.0000% differing (0/128000 px), max delta 0
frame 5859: 10.9750% differing (14048/128000 px), max delta 255
verdict: different, mode: mixed, offsets {"0": 4}
exit: 1
```

Re-run (`out/alku-recapture2`, exit 0 as a capture):

```
frame 900:  0.0000% differing (0/128000 px), max delta 0
frame 1719: 0.0000% differing (0/128000 px), max delta 0
frame 2719: 0.0000% differing (0/128000 px), max delta 0
frame 5859: 15.2797% differing (19558/128000 px), max delta 255
verdict: different, mode: mixed, offsets {"0": 4}
exit: 1
```

**What the picture difference is, measured.** The three `sha256`s that share an index are not three
unrelated pictures: they are one picture at different pan positions. Comparing the reference's 5859
against each partner over the picture band (rows 50–349, where the part's window sits; rows 0–49 and
350–399 are black in all three frames):

| partner | differing at offset 0 | best shift | differing after the shift |
|---|---|---|---|
| `out/alku-recapture` 5858, 5859, 5860 (one `sha256`, `a77b4991…`) | 14,048 px | 1 px horizontally | **0 px of 95,700** |
| `out/alku-recapture2` 5858, 5859, 5860 (one `sha256`, `069964a4…`) | 19,558 px | 2 px horizontally | **0 px of 95,700** |

So the sessions differ by a whole number of pixels of the part's own horizontal pan, and by nothing
else: the four sessions that took an index in this window produced three distinct pictures — the
reference's (5859 in the reference and the probe, 5860 in `out/b3-alku-boundary1`), one 1 pixel
along, and one 2 pixels along. The part advances that
pan one pixel per scroll step (`MAIN.C:7`, `:87`), nine frames apart, which is why the difference is
*pixel-exact* and why the ±1 support window cannot reach it: the reference's picture is a whole
number of steps away in time, not one frame. `out/alku-recapture2`'s extra frame says the same thing
from the other side — its 5850 is `a77b4991…`, the *other* partner's picture, and 5858 is
`069964a4…`, one pixel along: within one session the pan moved once in those eight frames.

**This is the phase's first moving-row evidence, and it is a different animal from D17's.** D17's
moving case leaves a 0.2–0.6 % residue because its one-frame shift is not uniform across the
picture; here nothing is left over at all once a *whole* number of pixels is removed, because the
part's motion is a discrete step. `compare` exits 1 and is right to: the bytes at the reference
index really do differ between sessions. No tolerance was added.

The columns this puts Phase 2 in: alku is compared in `mode: mixed` — 900 by indices (its palette
holds still), 1719, 2719 and 5859 by RGB, where the DAC's widening rule is load-bearing. And a port
of ALKU has an unambiguous target for 900, 1719 and 2719 (reproduce the bytes) but not for 5859
(reproduce the reference's pan position, which the oracle's own sessions do not agree on).

### 4.5 Where the part ends, and the transition

`out/b3-alku-sweep3` (5420…8120 step 300) was refused at its fourth index:

```
error: the display changed geometry between frames: frame 6020 is 320x200 where the capture
started at 320x400; capture one screen at a time
exit: 2
```

Narrowed with sparse single-index probes from both sides — `out/b3-alku-boundary1`
(5800, 5860, 5920, 5980) captured three and was refused at **5980**; `out/b3-alku-boundary2`
(5940, 5950, 5960, 5970) captured two and was refused at **5960** — so the change lies in
**(5950, 5960]**. No index in this window refused as a *miss*; the misses here are at the other end
of the part (404–410, above).

The last frames of the 320×400 screen are a transition, not the picture: 5800, 5860, 5920 and 5940
all have the same row structure — rows 50–349 picture, black above and below — while **5950** has
rows 0–24 black, 25–174 picture, 175–184 black, 185–349 picture, 350–399 black: the window is being
split and shifted as the next part takes the screen. No keyframe is taken in it, and 5950 is the
last 320×400 index sampled. The next part (U2A) starts at 5960 or 5961 and is 320×200 — §5 is its
row.

---

## 5. U2A

`SECOND 1`'s second part: a held "landing pad" scene, then a flying saucer over the same
landscape, then the landscape alone, then a second view of it. 320×200, one geometry, from 5960 or
5961 to a mode change in **(7810, 7950]** — about 27 seconds. It is the first part in the phase
whose screen the oracle reads as *indices* for some of its frames and as RGB for others (§5.3), and
the first whose keyframes include moving ones that still re-capture byte-identically.

The source is `VISU/C/U2A.C`: it loads its scene data (`U2A.0xx`) and a background picture, waits
for the music to reach one position (`VISU/C/U2A.C:227-239`, a `for(;;)` around `int 0fch` with
`a>10 && b>46`), sets the screen up (`vid_init(3) ////// oversample x 4` at `:242`,
`vid_window(0L,319L,25L,174L,512L,…)` at `:247`), and then runs a four-buffer animation loop
(`cl[0..3]`, `copper2` at `:105-130`, the loop at `:259-…`) that ends when the music reaches a
second position (`a>11 && b>54`, `:276`) — `xit` then unwinds it.

### 5.1 The sweeps, and what the sheets showed

```
out/b3-u2a-sweep1   5960,6300,6600,6900,7200,7500,7800,8100,8400,8700   exit 2, refused at 6300
out/b3-u2a-sweep2   6300,7000,7700,8400,9100,9800                       exit 2, refused at 8400
out/b3-u2a-sweep3   5960,6200,6450,6700,6950,7200,7450,7700,7950,8100,8200   exit 2, refused at 7950
out/b3-u2a-scan     5960…7810 step 50 (38 indices)                      exit 0
```

The refusals are the boundary story (§5.4). The frames they left on disk, and the scan, give the
part's shape. Tiles, in the order each run laid them out:

- **sweep3's 6300** — the landscape with a **yellow wireframe grid** across its lower half: the
  "pad". **7000** and **7700** — the same landscape with no grid at all. (Sweep 2, another session,
  had 6300 as the pad and 7000 and 7700 plain, so the pad arrives and leaves at about the same
  place in both.)
- **the scan**, the part in one session, every 50th frame: 5960 plain landscape; **6010–6960 the
  pad, one `sha256` for all twenty samples** — a hold of at least 950 frames; 7010 plain; **7060,
  7110, 7160, 7210, 7260 a grey saucer that fills the frame and recedes**; 7310, 7360, 7410 a small
  blue-white craft shrinking against the sky; 7460, 7510 the plain landscape; 7560, 7610, 7660 the
  plain landscape again, one `sha256`; **7710 a flat `(251,251,251)` frame**, one colour, the only
  flush-white frame the phase has captured; 7760, 7810 the landscape with a glowing orange object
  on the horizon, and a picture band one row lower (rows 25–175 rather than 25–174).

So the sheet says: pad hold → saucer → plain hold → flash → second view. The pad hold and the plain
hold are where this part's keyframes belong, and only the saucer moves.

### 5.2 The keyframes, and why these

| index | what is on screen | frame files | `sha256` |
|---|---|---|---|
| 6500 | the **pad** — the landscape with the yellow wireframe grid, 950 frames into a hold that the scan shows running from 6010 to 6960. | `.rgb` only | rgb `11a66eb0…` |
| 7300 | the **saucer receding** — a small blue-white craft against the sky, mid-animation. | `.rgb` + `.idx` | rgb `0a7c2216…`, idx `b50f52f4…` |
| 7640 | the **plain landscape**, inside a hold the scan shows running from 7560 to 7660. | `.rgb` + `.idx` | rgb `0a68ffb9…`, idx `c51ba997…` |
| 7760 | the **second view** — the landscape with a glowing orange object on the horizon, and the band one row lower. | `.rgb` only | rgb `5d558081…` |

Two of the four are held screens and two move, which is the split this part actually has. 6500 is
chosen deep inside the pad's 950-frame hold rather than at its edge (6010), because the hold's first
frames are within a few tens of frames of U2A's own mode change — an index that close to it is not
safe to put in a reference (§5.5). 7760 is at the far end for the same reason in reverse: it is the
last thing sampled before the boundary and it is the part's second view.

Full hashes of the six files in `reference/parts/u2a/`:

| index | file | `sha256` |
|---|---|---|
| 6500 | `frame_006500.rgb` | `11a66eb08a14b3cf65528ff982c272791393af06d9afacd3a2e85eb1dc81cf7e` |
| 7300 | `frame_007300.idx` | `b50f52f4e5b2389f281cddce87031ec9da2fd90e50a30edd6df2d5115703bd24` |
| 7300 | `frame_007300.rgb` | `0a7c2216cb3a7964d0221af01cfa07e8aab5372f39d9ef4f097e270decf3d590` |
| 7640 | `frame_007640.idx` | `c51ba997410ec820a3695e18bfe0622a856e65c0163b60dc83b9addff5753f1c` |
| 7640 | `frame_007640.rgb` | `0a68ffb92f0e208a3eb34ace29fa1ea91a51bebfea3ec2733ca30cd5d8b5bbd6` |
| 7760 | `frame_007760.rgb` | `5d558081c851f3f1b8a01ef777834f83757308a01207a23290d890ccdee77378` |

### 5.3 Held or moving, measured — and which frames carry indices

```bash
# the reference: keyframes only
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 6500,7300,7640,7760 --out reference/parts/u2a --sheet out/u2a.png

# the partner: a support frame each side of every keyframe, so the held/moving question is measured
cargo xtask reference capture --arg 1 --boot autoexec --keys "KBD_left KBD_enter" --cycles 30000 \
    --frames 6499,6500,6501,7299,7300,7301,7639,7640,7641,7759,7760,7761 --out out/u2a-recapture
```

The partner's triples, hashed:

| keyframe | `K-1` | `K` | `K+1` | reading |
|---|---|---|---|---|
| 6500 | `11a66eb0…` | `11a66eb0…` | `11a66eb0…` | **held** — one `sha256` across the triple |
| 7300 | `9f5fe99c…` | `0a7c2216…` | `0a7c2216…` | **moving** — the frame changes between 7299 and 7300 |
| 7640 | `0a68ffb9…` | `0a68ffb9…` | `0a68ffb9…` | **held** — one `sha256` |
| 7760 | `23fa5096…` | `5d558081…` | `5d558081…` | **moving** — changes between 7759 and 7760 |

So 6500 and 7640 need no support frames, and 7300 and 7760 need theirs — and all twelve frames were
captured anyway, which is what turns this table from an assumption into a measurement.

**Which frames carry `.idx` is not a property of held-or-moving here.** In the same run, 7639,
7640, 7641 and 7300 kept their indices and 6499–6501 and 7759–7761 did not; across the scan the
plain-landscape frames at 7010 and 7560–7660 kept them and every sample of the pad (6010–6960) and
of the second view (7760, 7810) did not. Reading the planes from offset 0 (`xtask/src/oracle/
dosbox.rs:618-629`) only matches the screen when the *displayed* buffer is the one at offset 0, and
this part rotates four animation buffers through its display start (`VISU/C/U2A.C:105-130`,
`vid_setswitch(-1,clr)` inside `copper2`); the measurements above are consistent with that and are
not evidence of motion.
Either way the RGB is the frame that was asked for: it is the same bytes in every session.

### 5.4 The re-capture: exit 0, identical, moving frames included

```
$ cargo xtask compare --reference reference/parts/u2a --rendered out/u2a-recapture \
      --out out/u2a-recapture-diff
frame 6500: 0.0000% differing (0/64000 px), max delta 0
frame 7300: 0.0000% differing (0/64000 px), max delta 0
frame 7640: 0.0000% differing (0/64000 px), max delta 0
frame 7760: 0.0000% differing (0/64000 px), max delta 0
verdict: identical, mode: mixed, offsets {"0": 4}
exit: 0
```

All four frames are byte-identical between the two directories, including the two that move. **That
is the same-alignment-group case, and it should be read as one**: D17 measured that two sessions
land in the same group about half the time, and this pair did. It is the first time in the phase
that a moving screen has re-captured this way, and it is a stronger claim than ALKU's picture could
make (§4.4) — but it is not a claim that this part's animation is session-independent, and nothing
here tested the other group. The held pair (6500, 7640) is what carries the reproducibility;
the moving pair is what shows the same group exists for this part too.

**And the animation is coarse enough that the alignment has something to bite on**: 7300's picture
changed from the frame before it and held for the frame after, so a session one frame out of phase
would find its answer at offset ±1 — exactly the case the support frames exist for. The other
group was not observed, so this is a statement about the frames' spacing, not about a result.

### 5.5 Where the part ends

```
out/b3-u2a-sweep3 (5940…, 320×200)     refused at 7950: "frame 7950 is 320x400 where the capture
                                       started at 320x200; capture one screen at a time"   exit 2
out/b3-u2a-sweep2 (6300…, 320×200)     refused at 8400: same refusal                         exit 2
out/b3-u2a-scan    (5960…7810, 320×200) exit 0 — 7810 is 320×200
```

So the change to the next part lies in **(7810, 7950]**, and 7810 is inside U2A. The window is
wider than ALKU's because the two bracketting indices come from two sessions, and the *frame* of a
mode change is not perfectly stable across sessions — measured at ALKU's end, where 5960 was
320×400 in `out/b3-alku-sweep1` and 320×200 in `out/b3-u2a-scan` (§4.5). Nothing between 7810 and
7950 was captured; the part's last sampled frame is 7810.

**U2A's own start is the same kind of fuzziness**, and it is worth stating because it constrains
where U2A's keyframes can go: 5960 is 320×200 plain landscape in `out/b3-u2a-scan` and still
320×400 ALKU in `out/b3-alku-sweep1`. An index in the first few tens of frames of a part can belong
to either side of a mode change depending on the session, so no reference keyframe should sit
there — which is why U2A's earliest keyframe is 6500, 490 frames clear of it.
